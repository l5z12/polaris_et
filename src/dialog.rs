// SPDX-License-Identifier: GPL-3.0-only
//! Thin wrappers over the Win32 common item dialogs (`IFileOpenDialog` /
//! `IFileSaveDialog`) for import/export. Runs on the UI thread; the WinUI host
//! has already initialized COM (STA).

use std::path::PathBuf;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, SIGDN_FILESYSPATH,
};
use windows::core::PCWSTR;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Owned name/spec buffers + the `COMDLG_FILTERSPEC`s that borrow them.
type FilterSpecs = (Vec<(Vec<u16>, Vec<u16>)>, Vec<COMDLG_FILTERSPEC>);

/// Owned backing buffers for a set of `COMDLG_FILTERSPEC`s.
fn build_specs(filters: &[(&str, &str)]) -> FilterSpecs {
    let bufs: Vec<(Vec<u16>, Vec<u16>)> = filters.iter().map(|(n, s)| (wide(n), wide(s))).collect();
    let specs = bufs
        .iter()
        .map(|(n, s)| COMDLG_FILTERSPEC {
            pszName: PCWSTR(n.as_ptr()),
            pszSpec: PCWSTR(s.as_ptr()),
        })
        .collect();
    (bufs, specs)
}

unsafe fn shell_item_path(item: &IShellItem) -> Option<PathBuf> {
    let raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
    let path = unsafe { raw.to_string() }.ok().map(PathBuf::from);
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    path
}

/// Show a "Save As" dialog. Returns the chosen path, or `None` if cancelled.
pub fn save_file(default_name: &str, filters: &[(&str, &str)]) -> Option<PathBuf> {
    save_file_typed(default_name, filters).map(|(p, _)| p)
}

/// Like [`save_file`], but also returns the 1-based index of the file type the
/// user selected — needed when two filters share an extension (e.g. a Polaris
/// `*.json` and an EasyTier `*.json`) and the path alone can't disambiguate.
pub fn save_file_typed(default_name: &str, filters: &[(&str, &str)]) -> Option<(PathBuf, u32)> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let (_bufs, specs) = build_specs(filters);
        let _ = dlg.SetFileTypes(&specs);
        let name = wide(default_name);
        let _ = dlg.SetFileName(PCWSTR(name.as_ptr()));
        dlg.Show(None).ok()?;
        let idx = dlg.GetFileTypeIndex().unwrap_or(1);
        let item = dlg.GetResult().ok()?;
        shell_item_path(&item).map(|p| (p, idx))
    }
}

/// Show an "Open" dialog. Returns the chosen path, or `None` if cancelled.
pub fn open_file(filters: &[(&str, &str)]) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let (_bufs, specs) = build_specs(filters);
        let _ = dlg.SetFileTypes(&specs);
        dlg.Show(None).ok()?;
        let item = dlg.GetResult().ok()?;
        shell_item_path(&item)
    }
}

/// Write `text` to the Windows clipboard as UTF-16. Runs on the UI thread (COM
/// is already initialized). Returns `false` if the clipboard couldn't be set.
pub fn write_clipboard_text(text: &str) -> bool {
    const CF_UNICODETEXT: u32 = 13;
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        // On success `SetClipboardData` takes ownership of `hmem`, so we only
        // free it on the failure paths.
        let ok = (|| {
            EmptyClipboard().ok()?;
            let hmem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).ok()?;
            let dst = GlobalLock(hmem) as *mut u16;
            if dst.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
            let _ = GlobalUnlock(hmem);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0))).ok()?;
            Some(())
        })()
        .is_some();
        let _ = CloseClipboard();
        ok
    }
}

/// Read UTF-16 text from the Windows clipboard. Runs on the UI thread (COM is
/// already initialized). Returns `None` if there is no text or it can't open.
pub fn read_clipboard_text() -> Option<String> {
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        OpenClipboard(None).ok()?;
        let text = read_clipboard_inner(CF_UNICODETEXT);
        let _ = CloseClipboard();
        text
    }
}

unsafe fn read_clipboard_inner(format: u32) -> Option<String> {
    unsafe {
        let handle: HANDLE = GetClipboardData(format).ok()?;
        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        let _ = GlobalUnlock(hglobal);
        Some(text)
    }
}

// Keep the unused-import lint quiet if HWND ends up unreferenced across versions.
#[allow(dead_code)]
fn _hwnd_marker(_: HWND) {}
