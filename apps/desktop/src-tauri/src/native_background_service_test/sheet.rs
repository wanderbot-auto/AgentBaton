//! Read-only observation and actual click of this process's exact attached sheet.
use objc2::{rc::Retained, runtime::AnyObject, MainThreadMarker, Message};
use objc2_app_kit::{NSButton, NSTextField, NSView, NSWindow};
use objc2_foundation::NSRect;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tauri::WebviewWindow;

fn contains(a: NSRect, b: NSRect) -> bool {
    [
        a.origin.x,
        a.origin.y,
        a.size.width,
        a.size.height,
        b.origin.x,
        b.origin.y,
        b.size.width,
        b.size.height,
    ]
    .iter()
    .all(|n| n.is_finite())
        && b.size.width > 0.
        && b.size.height > 0.
        && b.origin.x >= a.origin.x - 0.5
        && b.origin.y >= a.origin.y - 0.5
        && b.origin.x + b.size.width <= a.origin.x + a.size.width + 0.5
        && b.origin.y + b.size.height <= a.origin.y + a.size.height + 0.5
}
fn visible(v: &NSView, s: &NSWindow) -> bool {
    !v.isHiddenOrHasHiddenAncestor()
        && v.window().is_some_and(|w| std::ptr::eq(&*w, s))
        && contains(v.visibleRect(), v.bounds())
}
pub(super) struct Observed {
    pub number: isize,
    pub ok: Retained<NSButton>,
    pub cancel: Retained<NSButton>,
}
pub(super) fn observe(caller: &WebviewWindow, message: &str) -> Result<Option<Observed>, String> {
    let _main = MainThreadMarker::new().ok_or("Own sheet observation not on main")?;
    let pointer = caller
        .ns_window()
        .map_err(|_| "Caller NSWindow unavailable")?;
    if pointer.is_null() {
        return Err("Caller NSWindow is null".into());
    }
    // SAFETY: framework owns this actual live window; retain only on main thread.
    let parent =
        unsafe { Retained::retain(pointer.cast::<NSWindow>()) }.ok_or("Caller retain failed")?;
    let Some(sheet) = parent.attachedSheet() else {
        return Ok(None);
    };
    if !sheet
        .sheetParent()
        .is_some_and(|p| std::ptr::eq(&*p, &*parent))
    {
        return Err("Foreign sheet".into());
    }
    if !parent.isVisible()
        || !sheet.isVisible()
        || !sheet
            .screen()
            .is_some_and(|s| contains(s.visibleFrame(), sheet.frame()))
    {
        return Ok(None);
    }
    let content = sheet.contentView().ok_or("Missing sheet content")?;
    content.displayIfNeeded();
    let mut queue = vec![content];
    let mut count = 0;
    let mut title = 0;
    let mut body = 0;
    let mut ok = None;
    let mut cancel = None;
    while let Some(view) = queue.pop() {
        count += 1;
        if count > 256 {
            return Err("Own sheet view bound".into());
        }
        let object: &AnyObject = &view;
        if let Some(text) = object.downcast_ref::<NSTextField>() {
            let value = text.stringValue();
            if value.len_utf16() > 8192 {
                return Err("Own sheet text bound".into());
            }
            let value = value.to_string();
            if value == message || value == "AgentBaton — Login background registration" {
                let bounds = text.bounds();
                let mut room = bounds;
                room.size.height = 1_000_000.;
                let size = text
                    .cell()
                    .ok_or("Unmeasurable text")?
                    .cellSizeForBounds(room);
                if !visible(text, &sheet)
                    || !size.height.is_finite()
                    || !size.width.is_finite()
                    || size.height > bounds.size.height + 0.5
                    || size.width > bounds.size.width + 0.5
                {
                    return Ok(None);
                }
                if value == message {
                    body += 1;
                } else {
                    title += 1;
                }
            }
        }
        if let Some(button) = object.downcast_ref::<NSButton>() {
            let name = button.title();
            if name.len_utf16() > 128 {
                return Err("Button title bound".into());
            }
            let name = name.to_string();
            if name == "OK" || name == "Cancel" {
                if !button.isEnabled() || !visible(button, &sheet) {
                    return Ok(None);
                }
                let target = if name == "OK" { &mut ok } else { &mut cancel };
                if target.replace(button.retain()).is_some() {
                    return Err("Ambiguous real sheet button".into());
                }
            }
        }
        for child in &view.subviews() {
            if count + queue.len() >= 256 {
                return Err("Own sheet traversal bound".into());
            }
            queue.push(child.retain());
        }
    }
    if title != 1 || body != 1 {
        return Ok(None);
    }
    Ok(Some(Observed {
        number: sheet.windowNumber(),
        ok: ok.ok_or("No real OK")?,
        cancel: cancel.ok_or("No real Cancel")?,
    }))
}
pub(super) struct Capture {
    child: Child,
    deadline: Instant,
    path: PathBuf,
    pub number: isize,
    pub index: usize,
    pub page: usize,
}
impl Capture {
    pub fn start(output: &Path, index: usize, page: usize, number: isize) -> Result<Self, String> {
        if number <= 0 {
            return Err("Invalid attached sheet number".into());
        }
        let path = output.join(format!("step-{index:02}-page-{page:02}.png"));
        if path.exists() {
            return Err("Capture target already exists".into());
        }
        let child = Command::new("/usr/sbin/screencapture")
            .env_clear()
            .args(["-x", "-l", &number.to_string()])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Own-window capture spawn")?;
        Ok(Self {
            child,
            deadline: Instant::now() + Duration::from_secs(5),
            path,
            number,
            index,
            page,
        })
    }
    pub fn poll(&mut self) -> Result<bool, String> {
        match self.child.try_wait().map_err(|_| "Capture wait")? {
            Some(code) if code.success() => {
                use std::io::Read;
                let meta = std::fs::symlink_metadata(&self.path).map_err(|_| "Capture metadata")?;
                if !meta.is_file() || meta.len() < 8 || meta.len() > 16 * 1024 * 1024 {
                    return Err("Capture size/type".into());
                }
                let mut signature = [0; 8];
                std::fs::File::open(&self.path)
                    .map_err(|_| "Capture file")?
                    .read_exact(&mut signature)
                    .map_err(|_| "Capture header")?;
                if signature != *b"\x89PNG\r\n\x1a\n" {
                    return Err("Capture is not PNG".into());
                }
                Ok(true)
            }
            Some(_) => Err("Own sheet capture failed; no permission fallback".into()),
            None if Instant::now() >= self.deadline => Err("Own sheet capture deadline".into()),
            None => Ok(false),
        }
    }
    pub fn stop(&mut self) -> bool {
        if self.child.try_wait().is_ok_and(|s| s.is_some()) {
            return true;
        }
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.child.try_wait().is_ok_and(|s| s.is_some()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
