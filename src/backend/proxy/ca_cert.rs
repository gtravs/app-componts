use tracing::info;

use super::prelude::*;
use std::process::Command;
use std::path::Path;
use std::os::windows::process::CommandExt;
const CREATE_NO_WINDOW: u32 = 0x08000000;
// 生成自定义CA证书
pub async fn generate_ca_certificate() -> Result<CertifiedKey, anyhow::Error> {
    // // 证书已存在并加载
    if std::path::Path::new("ca.crt").exists() && std::path::Path::new("ca.key").exists() {
       //println!("[+] Load existing CA certificate and key");
        let cert_pem = std::fs::read_to_string("ca.crt").expect("[-] Failed read ca.crt");
        let key_pem = std::fs::read_to_string("ca.key").expect("[-] Failed read ca.key");
        let params = CertificateParams::from_ca_cert_pem(&cert_pem)?;
        
        let key_pair = KeyPair::from_pem(&key_pem).expect("[-] Failed to parse ca.key");
        let cert = params.self_signed(&key_pair)?;
        return Ok(CertifiedKey { cert, key_pair });
    }
    let ca_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let mut params = CertificateParams::new(vec!["GT TRAV CA".to_string()])?;
    // 设置 CA 证书的关键属性
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    // 设置证书有效期（例如：10年）
    params.not_before = std::time::SystemTime::now().into();
    params.not_after = (std::time::SystemTime::now() + std::time::Duration::from_secs(3650 * 24 * 60 * 60)).into();
    
    // 设置证书信息
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::OrganizationName, "GT TRAV CA");
    params.distinguished_name.push(DnType::CommonName, "GT TRAV CA");
    params.distinguished_name.push(DnType::CountryName, "CN");  // 可选：添加国家代码
    
    let ca_cert = params.self_signed(&ca_key_pair)?;
    // 保存证书和私钥
    std::fs::write("ca.crt", ca_cert.pem())?;
    std::fs::write("ca.key", ca_key_pair.serialize_pem())?;

    println!("[+] Generated new CA certificate and saved to ca.crt");
    println!("[+] Please install ca.crt in your browser/system");
    Ok(CertifiedKey { cert: ca_cert, key_pair: ca_key_pair })
}

// 生成自定义服务器证书
pub  async fn generate_signed_cert(ca_cert: &Certificate, ca_key: &KeyPair, host: String) -> Result<CertifiedKey, anyhow::Error> {

    let server_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let mut params = CertificateParams::new(vec![host.clone()])?;
    params.is_ca = rcgen::IsCa::ExplicitNoCa;
    // 设置证书有效期（例如：10年）
    params.not_before = std::time::SystemTime::now().into();
    params.not_after = (std::time::SystemTime::now() + std::time::Duration::from_secs(3650 * 24 * 60 * 60)).into();
    // 设置证书用途
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![
        rcgen::ExtendedKeyUsagePurpose::ServerAuth,
        rcgen::ExtendedKeyUsagePurpose::ClientAuth,
    ];
    // 设置证书主体信息
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, host.clone());
    params.distinguished_name.push(DnType::OrganizationName, "GT TRAV Server");
    params.distinguished_name.push(DnType::CountryName, "CN");
    
    params.use_authority_key_identifier_extension = true; 
    let cert = params.signed_by(&server_key_pair, ca_cert, ca_key)?;
    Ok(CertifiedKey { cert, key_pair: server_key_pair })
}



// 异步安装证书函数
pub async fn install_ca_certificate() -> Result<(), anyhow::Error> {
    let cert_path = Path::new("ca.crt");
    
    // 检查证书文件是否存在
    if !cert_path.exists() {
        return Err(anyhow::anyhow!("证书文件不存在"));
    }
    info!("开始安装证书...");

    // Windows 系统使用 certutil 安装证书
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("certutil")
            .creation_flags(CREATE_NO_WINDOW)
            .args(&[
                "-addstore",
                "-f",
                "ROOT",
                cert_path.to_str().unwrap(),
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "安装证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        
        info!("证书安装成功");
    }

    // Linux 系统安装证书
    #[cfg(target_os = "linux")]
    {
        // 复制证书到系统证书目录
        let output = Command::new("sudo")
            .args(&[
                "cp",
                cert_path.to_str().unwrap(),
                "/usr/local/share/ca-certificates/gt_trav_ca.crt",
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "复制证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // 更新证书存储
        let output = Command::new("sudo")
            .args(&["update-ca-certificates"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "更新证书存储失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("证书安装成功");
    }

    // macOS 系统安装证书
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("security")
            .args(&[
                "add-trusted-cert",
                "-d",
                "-r", "trustRoot",
                "-k", "/Library/Keychains/System.keychain",
                cert_path.to_str().unwrap(),
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "安装证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("证书安装成功");
    }

    Ok(())
}

// 检查证书是否已安装
pub async fn is_cert_installed() -> Result<bool, anyhow::Error> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("certutil")
            .creation_flags(CREATE_NO_WINDOW)
            .args(&[
                "-store",
                "ROOT",
                "GT TRAV CA"
            ])
            .output()?;

        Ok(!String::from_utf8_lossy(&output.stdout)
            .contains("未找到证书"))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(Path::new("/usr/local/share/ca-certificates/gt_trav_ca.crt").exists())
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("security")
            .args(&[
                "find-certificate",
                "-c", "GT TRAV CA",
                "-a"
            ])
            .output()?;

        Ok(output.status.success())
    }
}

// 卸载证书
pub async fn uninstall_ca_certificate() -> Result<(), anyhow::Error> {
    info!("开始卸载证书...");

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("certutil")
            .creation_flags(CREATE_NO_WINDOW)
            .args(&[
                "-delstore",
                "ROOT",
                "GT TRAV CA"
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "卸载证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 删除证书文件
        let output = Command::new("sudo")
            .args(&[
                "rm",
                "/usr/local/share/ca-certificates/gt_trav_ca.crt"
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "删除证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // 更新证书存储
        let output = Command::new("sudo")
            .args(&["update-ca-certificates"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "更新证书存储失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("security")
            .args(&[
                "delete-certificate",
                "-c", "GT TRAV CA"
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "卸载证书失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    info!("证书卸载成功");
    Ok(())
}
