use actix_web::middleware::DefaultHeaders;

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer#security_requirements
// This is a fix for https://github.com/ffmpegwasm/react-app/issues/3
// Reading:
//  - https://stackoverflow.com/questions/73275184/enable-shared-array-buffer-in-cross-domain
//  - https://blog.logrocket.com/understanding-sharedarraybuffer-and-cross-origin-isolation/
pub fn shared_array_buffer_cors() -> DefaultHeaders {
  DefaultHeaders::new()
      .add(("Cross-Origin-Embedder-Policy", "require-corp"))
      .add(("Cross-Origin-Opener-Policy", "same-origin"))
      .add(("Cross-Origin-Resource-Policy", "cross-origin"))
}
