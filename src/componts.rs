#![allow(unused_mut)]
#![allow(unused_variables)]
#![allow(unused_parens)]
#![allow(dead_code)]
#![allow(unused_imports)]
use tokio::sync::broadcast;
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use i_slint_backend_winit::{WinitWindowAccessor, WinitWindowEventResult};
use slint::{ComponentHandle, Global, Model, ModelRc, PhysicalPosition, SharedString, SharedVector, VecModel};
use winit::event::{ElementState, MouseButton, WindowEvent};
use crate::backend::proxy::ca_cert::{generate_ca_certificate,install_ca_certificate,is_cert_installed};
use crate::{backend, App, Setting};
use crate::slint_generatedApp::{self, GlobalBasicSettings};
use rfd::FileDialog;

pub  fn init_window_controls<T1,T2>(app: &T1, setting: &T2)
where
    T1: ComponentHandle  + 'static,
    T2: ComponentHandle  + 'static,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T1>,
    for<'a> GlobalBasicSettings<'a>: Global<'a, T2>,
{
    // 只绑定一次事件处理器
    move_window(app);
    move_window(setting);

    let app_weak = app.as_weak().unwrap();
    let setting_weak = setting.as_weak().unwrap();
    move_window(&app_weak);
    move_window(&setting_weak);

    open_windows(&app_weak, &setting_weak);
    handle_event(&app_weak);
    maximize_windows(&app_weak);
    maximize_windows(&setting_weak);
    minimize_windows(&app_weak);
    minimize_windows(&setting_weak);
    close_windows(&app_weak, true);
    close_windows(&setting_weak, false);
    create_array(&app_weak);
    install_cert(&app_weak);


    let window_weak = app.as_weak();
    app.global::<GlobalBasicSettings>().on_add_tab(move |tab_name| {
        let window = window_weak.unwrap();
        add_tab(&window, tab_name);
    });

    let window_weak = app.as_weak();
    app.global::<GlobalBasicSettings>().on_remove_tab(move |tab_name| {
        let window = window_weak.unwrap();
        remove_tab(&window, tab_name);
    });

}



pub fn move_window<T: ComponentHandle + 'static>(window: &T) {
    let is_dragging = Arc::new(Mutex::new(false));
    let drag_start_position = Arc::new(Mutex::new((0.0, 0.0)));
    let last_cursor_position = Arc::new(Mutex::new((0.0, 0.0)));
    
    let is_dragging_clone = Arc::clone(&is_dragging);
    let drag_start_position_clone = Arc::clone(&drag_start_position);
    let last_cursor_position_clone = Arc::clone(&last_cursor_position);
    // 定义标题栏高度（像素）
    const TITLE_BAR_HEIGHT: f64 = 35.0;
    window.window().on_winit_window_event(move |_slint_window: &slint::Window, event: &WindowEvent| {
        match event {
            WindowEvent::CursorMoved { device_id, position } => {
                let mut last_cursor_pos = last_cursor_position_clone.lock().unwrap();
                *last_cursor_pos = (position.x, position.y);
                
                let is_dragging = is_dragging_clone.lock().unwrap();
                if *is_dragging {
                    let drag_start_pos = drag_start_position_clone.lock().unwrap();
                    let window_start_pos = _slint_window.position();
                    let delta_x = position.x - drag_start_pos.0;
                    let delta_y = position.y - drag_start_pos.1;
                    let new_x = window_start_pos.x + delta_x as i32;
                    let new_y = window_start_pos.y + delta_y as i32;
                    drop(drag_start_pos);
                    let _ = window_start_pos;
                    _slint_window.set_position(PhysicalPosition::new(new_x, new_y));
                }
                WinitWindowEventResult::Propagate
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                if *button == MouseButton::Left {
                    let last_cursor_pos = last_cursor_position_clone.lock().unwrap();
                    // 检查鼠标是否在标题栏区域内
                    let is_in_title_bar = last_cursor_pos.1 <= TITLE_BAR_HEIGHT;
                    
                    let mut is_dragging = is_dragging_clone.lock().unwrap();
                    if *state == ElementState::Pressed && is_in_title_bar {
                        *is_dragging = true;
                        let mut drag_start_pos = drag_start_position_clone.lock().unwrap();
                        *drag_start_pos = *last_cursor_pos;
                    } else {
                        *is_dragging = false;
                    }
                    WinitWindowEventResult::Propagate
                } else {
                    let mut is_dragging = is_dragging_clone.lock().unwrap();
                    *is_dragging = false;
                    WinitWindowEventResult::Propagate
                }
            },
            _ => WinitWindowEventResult::Propagate,
        }
    });
}

// 关闭窗口
pub fn close_windows<'a, T: ComponentHandle + 'static>(window: &'a T,is_main_windows:bool) 
where
    GlobalBasicSettings<'a>: slint::Global<'a, T>
{
    let window_weak = window.as_weak();
    let global_settings = window.global::<GlobalBasicSettings>();
    global_settings.on_closewindow(move || {
        if let Some(handle) = window_weak.upgrade() {
            if is_main_windows {
                println!("[+] 主程序已关闭");
                std::process::exit(0);
            }
            handle.hide().unwrap();
        }
    });
}

// 打开窗口
pub fn open_windows<'a, T1: ComponentHandle + 'static, T2: ComponentHandle + 'static>(
    app: &'a T1,
    window: &'a T2
)
where
    GlobalBasicSettings<'a>: slint::Global<'a, T1>
{
    let global_settings = app.global::<GlobalBasicSettings>();
    let app_weak = app.as_weak().unwrap();
    let window_weak = window.as_weak();
    global_settings.on_openwindow(move || {
        if let Some(handle) = window_weak.upgrade() {
            handle.show().unwrap();
        }
    });
    
}


// 最大化窗口
pub fn maximize_windows<'a, T1: ComponentHandle + 'static>(window: &'a T1) -> bool
where
    GlobalBasicSettings<'a>: slint::Global<'a,T1> 
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let window_weak = window.as_weak();
    global_settings.on_maximize_window(move || {
        if let Some(handle) = window_weak.upgrade() {
            if !handle.window().is_maximized() {
                handle.window().set_maximized(true);
                return true;
            }else if handle.window().is_maximized() {
                handle.window().set_maximized(false);
                return false;
            }   
        }
        false
    });
    false 
}

// 最小化窗口
pub fn minimize_windows<'a, T1: ComponentHandle + 'static>(window: &'a T1) 
where
    GlobalBasicSettings<'a>: slint::Global<'a,T1>
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let window_weak = window.as_weak();
    global_settings.on_minimize_window(move || {
        if let Some(handle) = window_weak.upgrade() {
            if !handle.window().is_minimized() {
                handle.window().set_minimized(true);
            }else if handle.window().is_minimized() {
                handle.window().set_minimized(false);
            }
            
        }
    });
}


static PROXY_RUNNING: AtomicBool = AtomicBool::new(false);
static mut SHUTDOWN_SENDER: Option<broadcast::Sender<()>> = None;

pub fn handle_event<'a, T1: ComponentHandle + 'static>(window: &'a T1) 
where
    GlobalBasicSettings<'a>: slint::Global<'a,T1>
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let window_weak = window.as_weak();
    global_settings.on_handle_event(move |host:SharedString,port:SharedString| {
        if PROXY_RUNNING.load(Ordering::SeqCst) {
            // 停止代理
            PROXY_RUNNING.store(false, Ordering::SeqCst);
            unsafe {
                if let Some(sender) = &SHUTDOWN_SENDER {
                    let _ = sender.send(());
                }
            }

        } 
        else {
            // 启动代理
            PROXY_RUNNING.store(true, Ordering::SeqCst);
            let (tx, _) = broadcast::channel(1);
            unsafe {
                SHUTDOWN_SENDER = Some(tx.clone());
            }

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = backend::proxy::http_proxy::entry(tx,host.to_string(),port.to_string()).await {
                        eprintln!("Proxy error: {}", e);
                    }
                });
            });
        }
        // std::thread::spawn(|| {
        //     let rt =   tokio::runtime::Runtime::new().unwrap();
        //     rt.block_on(async  {
        //         backend::proxy::http_proxy::entry().await;
        //     })

        // });
    });
}

pub fn add_tab<'a, T1: ComponentHandle + 'static>(window: &'a T1, tab_name: slint::SharedString) 
where
    GlobalBasicSettings<'a>: slint::Global<'a, T1> 
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let current_tabs = global_settings.get_visible_tabs();
    
    // 将 ModelRc 转换为 Vec
    let mut tabs_vec: Vec<slint::SharedString> = current_tabs.iter().collect();
    
    // 检查是否已存在
    if !tabs_vec.iter().any(|tab| tab == &tab_name) {
        tabs_vec.push(tab_name.clone());
        // 使用 VecModel
        let vec_model = std::rc::Rc::new(VecModel::from(tabs_vec));
        global_settings.set_visible_tabs(vec_model.into());
        global_settings.set_active_tab(tab_name);
    }
}

pub fn remove_tab<'a, T1: ComponentHandle + 'static>(window: &'a T1, tab_name: slint::SharedString) 
where
    GlobalBasicSettings<'a>: slint::Global<'a, T1> 
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let current_tabs = global_settings.get_visible_tabs();
    
    // 不允许删除 Event Log
    if tab_name == "Event Log" {
        return;
    }

    // 如果要删除的是当前活动标签，先找到前一个标签
    if global_settings.get_active_tab() == tab_name {
        let current_index = current_tabs.iter()
            .position(|tab| tab == &tab_name)
            .unwrap_or(0);
        
        // 如果存在前一个标签，切换到前一个；否则切换到 Event Log
        let new_active_tab = if current_index > 0 {
            current_tabs.iter()
                .nth(current_index - 1)
                .unwrap_or(slint::SharedString::from("Event Log"))
                .clone()
        } else {
            slint::SharedString::from("Event Log")
        };
        
        global_settings.set_active_tab(new_active_tab);
    }

    // 过滤出要保留的标签
    let tabs_vec: Vec<slint::SharedString> = current_tabs.iter()
        .filter(|tab| tab != &&tab_name)
        .collect();

    // 使用 VecModel
    let vec_model = std::rc::Rc::new(VecModel::from(tabs_vec));
    global_settings.set_visible_tabs(vec_model.into());
}


pub fn create_array<'a, T1: ComponentHandle + 'static>(window: &'a T1)
where
    GlobalBasicSettings<'a>: slint::Global<'a,T1>  
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let window_weak = window.as_weak();

    global_settings.on_create_array(move |lt:i32,s:f32| {
        let array = vec![s;lt as usize];
        Rc::new(VecModel::from(array)).into()
    });
}

pub  fn install_cert<'a, T1: ComponentHandle + 'static>(window: &'a T1)
where
    GlobalBasicSettings<'a>: slint::Global<'a,T1>  
{
    let global_settings = window.global::<GlobalBasicSettings>();
    let window_weak = window.as_weak();
    global_settings.on_install_cert(move || {
        tokio::spawn(async move {
            if  let Ok(cacert) = generate_ca_certificate().await {
                // 获取证书和私钥的 PEM 格式
                let cert_chain = cacert.cert;
                let private_key = cacert.key_pair;
                // 保存证书
                if let Some(cert_path) = FileDialog::new()
                .set_title("保存证书文件")
                .set_file_name("ca.crt")
                .add_filter("证书文件", &["crt", "pem"])
                .save_file() 
            {
    
                if let Err(e) = std::fs::write(&cert_path, cert_chain.pem()) {
                    println!("保存证书失败: {}", e);
                    return;
                }
                println!("证书已保存到: {}", cert_path.display());
            }}
        });
    });

}



