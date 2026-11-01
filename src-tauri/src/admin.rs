//! Starting Encore as administrator without a Windows prompt every time.
//!
//! Games that use the whole graphics card starve the recording unless the capture gets raised
//! graphics priority, and Windows only allows that for administrators. Rather than asking every
//! time Encore opens, the person approves once: Encore registers a Windows scheduled task (no
//! schedule, never runs by itself) that starts Encore with the highest rights the account has.
//! When Encore is opened normally it starts that task and closes, so it comes back elevated with
//! no prompt. Turning the setting off removes the task (one more prompt).
//!
//! The switch is a marker file next to Encore's other data, read before anything else starts.
//! If the task has gone (removed by hand, say), Encore simply opens normally.

#![cfg_attr(not(windows), allow(dead_code))]

use std::path::PathBuf;

/// Name of the scheduled task, as it appears in Task Scheduler.
pub const TASK_NAME: &str = "Encore - start as administrator";
/// Passed to Encore when the task starts it, so it never tries to hand over a second time.
pub const FROM_TASK_ARG: &str = "--from-admin-task";

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Encore"))
}

fn marker() -> Option<PathBuf> {
    data_dir().map(|d| d.join("start-as-admin"))
}

pub fn wanted() -> bool {
    marker().map(|m| m.exists()).unwrap_or(false)
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn is_elevated() -> bool {
        use std::mem::{size_of, zeroed};
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elev: TOKEN_ELEVATION = zeroed();
            let mut len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elev as *mut _ as *mut std::ffi::c_void,
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            );
            CloseHandle(token);
            ok != 0 && elev.TokenIsElevated != 0
        }
    }

    fn schtasks(args: &[&str]) -> bool {
        Command::new("schtasks.exe")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Starts the task (which opens Encore elevated). False if it's missing or couldn't start.
    pub fn run_task() -> bool {
        schtasks(&["/Run", "/TN", TASK_NAME])
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    fn task_xml(exe: &str, user: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Opens Encore with administrator rights so game recordings stay smooth. Created when "Start Encore as administrator" was turned on in Encore's settings; turning it off removes this task.</Description>
  </RegistrationInfo>
  <Triggers />
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{arg}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
            exe = xml_escape(exe),
            user = xml_escape(user),
            arg = FROM_TASK_ARG,
        )
    }

    /// Runs schtasks with administrator rights: directly if Encore already has them, otherwise
    /// through Windows' "Do you want to allow…" prompt. Returns a reason on failure.
    fn schtasks_elevated(args: &[String]) -> Result<(), String> {
        if is_elevated() {
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            return if schtasks(&a) { Ok(()) } else { Err("Windows couldn't make the change.".into()) };
        }
        // One argument string for Start-Process, quoting the parts with spaces.
        let joined = args
            .iter()
            .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() })
            .collect::<Vec<_>>()
            .join(" ")
            .replace('\'', "''");
        let script = format!(
            "try {{ $p = Start-Process -FilePath 'schtasks.exe' -ArgumentList '{joined}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ErrorAction Stop; exit $p.ExitCode }} catch {{ exit 1223 }}"
        );
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        match status.code() {
            Some(0) => Ok(()),
            Some(1223) => Err("Windows didn't get permission, so nothing was changed.".into()),
            _ => Err("Windows couldn't make the change.".into()),
        }
    }

    pub fn enable() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
            (Ok(d), Ok(u)) => format!("{d}\\{u}"),
            (_, Ok(u)) => u,
            _ => return Err("Couldn't work out the Windows user name.".into()),
        };
        let dir = data_dir().ok_or("Couldn't find Encore's data folder.")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let xml_path = dir.join("admin-task.xml");
        // Task Scheduler expects UTF-16 with a byte-order mark.
        let mut bytes = vec![0xFF, 0xFE];
        for unit in task_xml(&exe, &user).encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&xml_path, bytes).map_err(|e| e.to_string())?;
        let xml = xml_path.to_string_lossy().to_string();
        let args = ["/Create", "/TN", TASK_NAME, "/XML", xml.as_str(), "/F"].map(String::from);
        let result = schtasks_elevated(&args);
        let _ = std::fs::remove_file(&xml_path);
        result?;
        std::fs::write(marker().ok_or("Couldn't find Encore's data folder.")?, b"on").map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        if let Some(m) = marker() {
            let _ = std::fs::remove_file(m);
        }
        // Remove the task too; if it's already gone (or the prompt is declined) the marker is
        // off anyway, so Encore stops using it.
        let _ = schtasks_elevated(&["/Delete", "/TN", TASK_NAME, "/F"].map(String::from));
        Ok(())
    }
}

/// Called first thing in main(). If the person turned on "Start Encore as administrator" and this
/// copy isn't elevated, start the task and tell the caller to exit. Returns true to exit.
pub fn hand_over_if_wanted() -> bool {
    #[cfg(windows)]
    {
        if std::env::args().any(|a| a == FROM_TASK_ARG) {
            // Started by the task: give the copy that started it a moment to close, so the
            // one-copy-only check doesn't mistake it for an Encore that's already open.
            std::thread::sleep(std::time::Duration::from_millis(400));
            return false;
        }
        if !wanted() || win::is_elevated() {
            return false;
        }
        crate::startup::remember_hidden_for_handover();
        win::run_task()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Whether Encore is elevated now, and whether it's set to start that way.
#[tauri::command]
pub fn admin_status() -> serde_json::Value {
    #[cfg(windows)]
    let elevated = win::is_elevated();
    #[cfg(not(windows))]
    let elevated = false;
    serde_json::json!({ "elevated": elevated, "start_as_admin": wanted() })
}

/// Turns "Start Encore as administrator" on or off. Windows asks for permission once.
#[tauri::command]
pub async fn set_start_as_admin(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || if on { win::enable() } else { win::disable() })
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Only available on Windows.".into())
    }
}

