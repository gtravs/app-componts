pub mod logs;
pub mod componts;
pub mod backend;
slint::include_modules!();

use std::sync::Arc;

use logs::http_proxylogs::{LogManager, LogSystem, GLOBAL_LOG_MANAGER};
use slint::{ComponentHandle, Global};
use tokio::time::Duration;

pub use slint_generatedApp::*; 

// 通用的应用初始化函数
pub async fn initialize_application<T1,T2>(
    main_window: T1,
    setting_window: T2,
    max_logs: usize,
    batch_size: usize,
    update_interval: Duration
) -> Result<(), slint::PlatformError> 
where
    T1: ComponentHandle  + 'static,
    T2: ComponentHandle  + 'static,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T1>,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T2>,
{
    let main_weak = main_window.as_weak().unwrap();
    let setting_weak = setting_window.as_weak().unwrap();

    // 初始化窗口控制
    componts::init_window_controls(&main_weak, &setting_weak);

    // 初始化日志系统
    let log_system= LogSystem::new(
        &main_window,
        max_logs,
        batch_size,
        update_interval,

    );
    // info!(
    //     "REQ001",
    //     "example.com",
    //     "GET",
    //     "/api/users",
    //     "200",
    //     "1024"
    // );
    // info!("[+] application launch");
    // 运行主窗口
    main_window.run()?;

    Ok(())
}