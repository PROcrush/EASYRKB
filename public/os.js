// Sets the OS before first paint so the per-OS font tokens apply without a
// flash of the wrong family.
//
// A separate file rather than an inline <script> so the Content-Security
// Policy can be `script-src 'self'`. Inline would need 'unsafe-inline' or a
// hash of this exact text, and a hash silently becomes a blank window the
// first time somebody reformats the file.
document.documentElement.dataset.os = navigator.userAgent.includes("Mac")
  ? "macos"
  : navigator.userAgent.includes("Win")
    ? "windows"
    : "linux";
