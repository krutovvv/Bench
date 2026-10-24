//! macOS notifications (UserNotifications): "the agent finished" while you were elsewhere.
//!
//! Only inside the .app: the notification center needs a bundle (a bare `target/` binary has
//! none, and asking for the center there aborts the process), so outside it this does nothing.
//! macOS asks the person once ("Bench would like to send notifications"); after that the
//! choice is theirs, in System Settings → Notifications.

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNNotificationSound, UNUserNotificationCenter,
};

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
    let done = RcBlock::new(|_granted: Bool, _err: *mut NSError| {});
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
