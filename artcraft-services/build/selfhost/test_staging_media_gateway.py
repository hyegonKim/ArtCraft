import importlib.util
import json
from pathlib import Path
import threading
import time
import unittest
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

spec = importlib.util.spec_from_file_location("gateway", Path(__file__).with_name("staging-media-gateway.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
ORIGIN = "https://private.example.test"
MEDIA_PATH = "/media/a/b/c/file.mp4"


class FakeBackend(BaseHTTPRequestHandler):
  calls = []

  def log_message(self, *args):
    pass

  def do_GET(self):
    self.calls.append(self.path)
    cookie = self.headers.get("Cookie")
    if self.headers.get("session") == "signed-owner":
      cookie = "session=owner"
    user = "user_owner" if cookie == "session=owner" else "user_other"
    if self.path == "/v1/session":
      data = {"logged_in": cookie in ("session=owner", "session=other"),
              "user": {"user_token": user, "username": user}}
    elif self.path.startswith("/v1/media_files/file/"):
      data = {"success": True, "media_file": {"token": "mf_owned", "maybe_creator_user": {"user_token": "user_owner"},
              "media_links": {"cdn_url": "https://legacy.test" + MEDIA_PATH, "maybe_video_previews": {"still": "missing"}}}}
    elif self.path.startswith("/buckets/"):
      content = b"video-bytes"
      partial = bool(self.headers.get("Range"))
      if partial:
        content = content[0:5]
      self.send_response(206 if partial else 200)
      self.send_header("Content-Type", "video/mp4")
      self.send_header("Content-Length", str(len(content)))
      if partial:
        self.send_header("Content-Range", "bytes 0-4/11")
      self.end_headers()
      self.wfile.write(content)
      return
    else:
      data = {"success": True, "results": []}
    body = json.dumps(data).encode()
    self.send_response(200)
    self.send_header("Content-Length", str(len(body)))
    self.end_headers()
    self.wfile.write(body)

  def do_POST(self):
    self.calls.append(self.path)
    length = int(self.headers.get("Content-Length", "0"))
    self.rfile.read(length)
    body = b'{"success":true,"media_file_token":"mf_saved"}'
    self.send_response(200)
    self.send_header("Content-Length", str(len(body)))
    self.end_headers()
    self.wfile.write(body)


class GatewayTests(unittest.TestCase):
  @classmethod
  def setUpClass(cls):
    cls.backend = ThreadingHTTPServer(("127.0.0.1", 0), FakeBackend)
    cls.server = ThreadingHTTPServer(("127.0.0.1", 0), module.Handler)
    cls.base = f"http://127.0.0.1:{cls.server.server_port}"
    cls.server.gateway = module.Gateway(f"127.0.0.1:{cls.backend.server_port}",
                                       f"127.0.0.1:{cls.backend.server_port}", ORIGIN, cls.base, b"x" * 32)
    for server in (cls.backend, cls.server):
      threading.Thread(target=server.serve_forever, daemon=True).start()

  @classmethod
  def tearDownClass(cls):
    for server in (cls.backend, cls.server):
      server.shutdown()
      server.server_close()

  def request(self, path, method="GET", cookie="session=owner", origin=ORIGIN, data=None, headers=None):
    request_headers = {"Origin": origin, "Cookie": cookie, **(headers or {})}
    request = urllib.request.Request(self.base + path, data=data, method=method, headers=request_headers)
    try:
      response = urllib.request.urlopen(request)
    except urllib.error.HTTPError as error:
      response = error
    with response:
      return response.status, response.headers, response.read()

  def test_anonymous_upload_and_project_listing_are_blocked(self):
    for path, method in (("/v1/media_files/upload/image", "POST"), ("/v1/media_files/project/list", "GET")):
      self.assertEqual(self.request(path, method, cookie="", data=b"" if method == "POST" else None)[0], 401)

  def test_untrusted_origin_is_blocked(self):
    self.assertEqual(self.request("/v1/media_files/project/list", origin="https://other.test")[0], 403)

  def test_upload_and_project_update_are_forwarded(self):
    for path in ("/v1/media_files/upload/image", "/v1/media_files/upload/new_video",
                 "/v1/media_files/upload/project/video_timeline/new",
                 "/v1/media_files/upload/project/scene_3d/update/mf_owned"):
      status, _, body = self.request(path, "POST", data=b"{}")
      self.assertEqual(status, 200)
      self.assertEqual(json.loads(body)["media_file_token"], "mf_saved")

  def test_other_user_cannot_read_or_update_project(self):
    for path, method in (("/v1/media_files/file/mf_owned", "GET"),
                         ("/v1/media_files/upload/project/video_timeline/update/mf_owned", "POST"),
                         ("/v1/media_files/rename/mf_owned", "POST")):
      self.assertEqual(self.request(path, method, cookie="session=other", data=b"{}" if method == "POST" else None)[0], 404)

  def test_global_list_is_replaced_by_owner_sql_list(self):
    self.assertEqual(self.request("/v1/media_files/list?page_size=20")[0], 200)
    self.assertEqual(FakeBackend.calls[-1], "/v1/media_files/list/user/user_owner?page_size=20")

  def test_signed_session_works_when_browser_blocks_third_party_cookies(self):
    self.assertEqual(self.request('/v1/media_files/file/mf_owned', cookie='', headers={'session': 'signed-owner'})[0], 200)
    self.assertEqual(self.request('/v1/media_files/project/list', cookie='', headers={'session': 'invalid'})[0], 401)

  def test_other_username_and_generation_routes_are_not_exposed(self):
    for path in ("/v1/media_files/list/user/user_other", "/v1/omni_gen/generate/image", "/v1/media_files/search_session"):
      self.assertEqual(self.request(path)[0], 404)

  def test_gallery_session_lists_require_auth_and_allow_only_reads(self):
    for path in module.SESSION_LISTS:
      if path.endswith('/batch'):
        continue
      self.assertEqual(self.request(path)[0], 200)
      self.assertEqual(self.request(path, cookie='')[0], 401)
      self.assertEqual(self.request(path, 'POST', data=b'{}')[0], 404)

  def test_links_are_rewritten_and_bytes_support_ranges(self):
    status, headers, body = self.request("/v1/media_files/file/mf_owned")
    self.assertEqual(status, 200)
    self.assertEqual(headers["Cache-Control"], "private, no-store")
    media = json.loads(body)["media_file"]
    self.assertIsNone(media["media_links"]["maybe_video_previews"])
    path = media["media_links"]["cdn_url"].removeprefix(self.base)
    self.assertTrue(path.split('?', 1)[0].endswith('/file.mp4'))
    status, headers, body = self.request(path, cookie="", headers={"Range": "bytes=0-4"})
    self.assertEqual((status, body), (206, b"video"))
    self.assertEqual(headers["Content-Range"], "bytes 0-4/11")
    self.assertEqual(headers["Access-Control-Allow-Origin"], ORIGIN)

  def test_tampered_and_expired_links_are_blocked(self):
    path = self.server.gateway.media_url("https://legacy.test" + MEDIA_PATH).removeprefix(self.base)
    self.assertEqual(self.request(path.replace("signature=", "signature=x"))[0], 403)
    expired = path.split("?", 1)[0] + "?expires=1&signature=" + self.server.gateway.sign_path(MEDIA_PATH, 1)
    self.assertEqual(self.request(expired)[0], 403)
    self.assertEqual(self.request("/staging-media/not-a-link")[0], 403)
    self.assertEqual(self.request(path.replace('/file.mp4', '/file.png'))[0], 403)

  def test_path_traversal_is_rejected(self):
    for path in ("/media/../private", "/media/%2e%2e/private", "/other/file", "/media/a\\b"):
      with self.assertRaises(ValueError):
        self.server.gateway.media_url("https://legacy.test" + path)

  def test_oversize_upload_is_rejected_before_reading(self):
    self.assertEqual(self.request("/v1/media_files/upload/image", "POST", data=b"", headers={"Content-Length": str(module.MAX_UPLOAD + 1)})[0], 413)

  def test_cover_requires_both_files_to_belong_to_session(self):
    self.assertEqual(self.request("/v1/media_files/cover_image/mf_owned", "POST", data=b'{}')[0], 400)
    self.assertEqual(self.request("/v1/media_files/cover_image/mf_owned", "POST", data=b'{"cover_image_media_file_token":"../other"}')[0], 404)

  def test_cors_preflight_and_secret_validation(self):
    status, headers, _ = self.request("/v1/media_files/upload/image", "OPTIONS", cookie="")
    self.assertEqual(status, 204)
    self.assertEqual(headers["Access-Control-Allow-Credentials"], "true")
    self.assertIn("session", headers["Access-Control-Allow-Headers"])
    self.assertEqual(self.request("/v1/media_files/upload/image", "OPTIONS", origin="https://other.test")[0], 403)
    with self.assertRaises(ValueError):
      module.Gateway("api", "filer", ORIGIN, self.base, b"weak")


if __name__ == "__main__":
  unittest.main()
