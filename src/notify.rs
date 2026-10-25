//! macOS notifications (UserNotifications): "the agent finished" while you were elsewhere.
//!
//! Only inside the .app: the notification center needs a bundle (a bare `target/` binary has
//! none, and asking for the center there aborts the process), so outside it this does nothing.
//! macOS asks the person once ("Bench would like to send notifications"); after that the
//! choice is theirs, in System Settings → Notifications.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, Ordering};

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
    UNNotificationRequest, UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
};

/// What macOS lets us do: 0 = not asked yet, 1 = allowed, 2 = refused. macOS answers on its own
/// thread, so this is an atomic and the interface reads it as it is.
pub static AUTH: AtomicU8 = AtomicU8::new(0);

fn remember(granted: bool, asked: bool) {
    AUTH.store(if asked { 0 } else if granted { 1 } else { 2 }, Ordering::Relaxed);
}

/// Refresh `AUTH` from what macOS says now (the person may have changed their mind in System
/// Settings without quitting Bench).
pub fn check() {
    if !bundled() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let done = RcBlock::new(|settings: NonNull<UNNotificationSettings>| {
        let st = unsafe { settings.as_ref() }.authorizationStatus();
        remember(
            st == UNAuthorizationStatus::Authorized || st == UNAuthorizationStatus::Provisional,
            st == UNAuthorizationStatus::NotDetermined,
        );
    });
    center.getNotificationSettingsWithCompletionHandler(&done);
}

/// Running from inside an .app bundle (the only place notifications can work).
fn bundled() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// Ask for permission once, at start (macOS shows its prompt only the first time).
pub fn request() {
    if !bundled() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    check();
    let done = RcBlock::new(|granted: Bool, _err: *mut NSError| {
        remember(granted.as_bool(), false);
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &done,
    );
}

/// Show a notification now. `id` replaces an earlier one with the same id (one per chat).
pub fn post(id: &str, title: &str, body: &str) {
    if !bundled() {
        return;
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(id),
        &content,
        None,
    );
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.addNotificationRequest_withCompletionHandler(&request, None);
}
