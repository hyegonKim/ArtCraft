
#[test]
fn test_dynamic_library_loading() {
  let name = libloading::library_filename("cublas");

  assert_eq!(name, "libcublas.so");
}