"""Private staging adapter for the legacy API and internal SeaweedFS filer.

Only this gateway is routed publicly for media APIs. Storage and the legacy
API stay on Docker's private network. Media links are short-lived bearer URLs;
never log their query strings or cookies. No third-party Python dependencies.
"""

import base64
import hashlib
import hmac
import http.client
import json
import logging
import os
import re
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlencode, urlsplit

MAX_UPLOAD = 64 * 1024 * 1024
MAX_JSON = 8 * 1024 * 1024
LINK_TTL = 1800
TOKEN = r"[A-Za-z0-9_]+"
PROJECT_KIND = r"(?:video_timeline|scene_3d|editor_2d|mood_board)"
UPLOAD = re.compile(rf"/v1/media_files/upload/(?:image|audio|new_video|studio_shot|project/{PROJECT_KIND}/new)")
UPDATE = re.compile(rf"/v1/media_files/upload/project/{PROJECT_KIND}/update/({TOKEN})")
FILE = re.compile(rf"/v1/media_files/(?:file|rename|cover_image)/({TOKEN})")
SESSION_LISTS = ("/v1/media_files/project/list", "/v1/media_files/list", "/v1/media_files/batch",
                 "/v1/media_files/mesh/list", "/v1/media_files/splat/list", "/v1/folders/list_all",
                 "/v1/folders/media_files_without_folder", "/v1/tags/list")
SLOTS = threading.BoundedSemaphore(4)


class Gateway:
  def __init__(self, api, filer, origin, public_origin, secret):
    if len(secret) < 32:
      raise ValueError("MEDIA_LINK_SECRET must contain at least 32 bytes")
    self.api = api
    self.filer = filer
    self.origin = origin
    self.public_origin = public_origin.rstrip("/")
    self.secret = secret

  def upstream(self, method, path, cookie="", body=None, content_type="application/json", signed_session=""):
    headers = {"Origin": self.origin, "Cookie": cookie, "Content-Type": content_type}
    if signed_session:
      headers["session"] = signed_session
    conn = http.client.HTTPConnection(self.api, timeout=120)
    try:
      conn.request(method, path, body=body, headers=headers)
      response = conn.getresponse()
      data = response.read(MAX_JSON + 1)
      if len(data) > MAX_JSON:
        raise ValueError("API response too large")
      return response.status, json.loads(data)
    finally:
      conn.close()

  def session(self, cookie, signed_session=""):
    if not cookie and not signed_session:
      return None
    status, data = self.upstream("GET", "/v1/session", cookie, signed_session=signed_session)
    if status != 200 or not data.get("logged_in"):
      return None
    return data.get("user")

  def owned_file(self, token, cookie, user, signed_session=""):
    status, data = self.upstream("GET", "/v1/media_files/file/" + token, cookie, signed_session=signed_session)
    media = data.get("media_file", {})
    owner = media.get("maybe_creator_user") or {}
    if status != 200 or owner.get("user_token") != user.get("user_token"):
      return None
    return media

  def sign_path(self, path, expires):
    return hmac.new(self.secret, f"{expires}\n{path}".encode(), hashlib.sha256).hexdigest()

  def media_url(self, raw):
    path = unquote(urlsplit(raw).path)
    if not safe_storage_path(path):
      raise ValueError("Invalid media storage path")
    encoded = base64.urlsafe_b64encode(path.encode()).decode().rstrip("=")
    expires = int(time.time()) + LINK_TTL
    query = urlencode({"expires": expires, "signature": self.sign_path(path, expires)})
    filename = quote(path.rsplit("/", 1)[1], safe="")
    # Retain the extension: existing viewers and asset loaders use it to
    # distinguish video/audio/3D assets from images.
    return f"{self.public_origin}/staging-media/{encoded}/{filename}?{query}"

  def rewrite(self, value):
    if isinstance(value, list):
      return [self.rewrite(item) for item in value]
    if not isinstance(value, dict):
      return value
    result = {}
    for key, item in value.items():
      if key in ("cdn_url", "public_bucket_url", "maybe_thumbnail_template", "thumbnail_template") and isinstance(item, str):
        result[key] = self.media_url(item)
      elif key == "maybe_video_previews":
        # The lightweight deployment has no asynchronous thumbnail worker.
        result[key] = None
      else:
        result[key] = self.rewrite(item)
    return result


def safe_storage_path(path):
  return (path.startswith("/media/") and not any(part in (".", "..", "") for part in path[1:].split("/"))
          and not any(char in path for char in ("\\", "\r", "\n", "\0", "?", "#")))


class Handler(BaseHTTPRequestHandler):
  server_version = "ArtCraftStaging"
  protocol_version = "HTTP/1.1"

  def log_message(self, format, *args):
    # BaseHTTPRequestHandler's default includes signed media URLs.
    logging.info("%s %s", self.command, urlsplit(self.path).path)

  def reply(self, status, data):
    body = json.dumps(data).encode()
    self.send_response(status)
    self.common_headers()
    self.send_header("Content-Type", "application/json")
    self.send_header("Content-Length", str(len(body)))
    self.send_header("Connection", "close")
    self.end_headers()
    self.close_connection = True
    if self.command != "HEAD":
      self.wfile.write(body)

  def common_headers(self):
    if self.headers.get("Origin") == self.server.gateway.origin:
      self.send_header("Access-Control-Allow-Origin", self.server.gateway.origin)
      self.send_header("Access-Control-Allow-Credentials", "true")
      self.send_header("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length")
    self.send_header("Vary", "Origin")
    self.send_header("Cache-Control", "private, no-store")
    self.send_header("X-Content-Type-Options", "nosniff")
    self.send_header("X-Robots-Tag", "noindex, nofollow")

  def do_OPTIONS(self):
    if self.headers.get("Origin") != self.server.gateway.origin:
      return self.reply(403, {"success": False})
    self.send_response(204)
    self.common_headers()
    self.send_header("Access-Control-Allow-Methods", "GET, HEAD, POST, DELETE, OPTIONS")
    self.send_header("Access-Control-Allow-Headers", "Content-Type, Range, session")
    self.send_header("Content-Length", "0")
    self.end_headers()

  def do_GET(self):
    self.handle_request()

  def do_HEAD(self):
    self.handle_request()

  def do_POST(self):
    self.handle_request()

  def do_DELETE(self):
    self.handle_request()

  def handle_request(self):
    if not SLOTS.acquire(blocking=False):
      return self.reply(503, {"success": False, "error": "Please retry shortly"})
    try:
      self.connection.settimeout(120)
      self.dispatch()
    except (BrokenPipeError, ConnectionResetError):
      pass
    except Exception:
      logging.exception("Media gateway request failed")
      self.reply(502, {"success": False, "error": "Storage request failed"})
    finally:
      SLOTS.release()

  def dispatch(self):
    gateway = self.server.gateway
    path = urlsplit(self.path).path
    if path.startswith("/staging-media/"):
      return self.serve_media()
    if self.headers.get("Origin") != gateway.origin:
      return self.reply(403, {"success": False})
    cookie = self.headers.get("Cookie", "")
    signed_session = self.headers.get("session", "")
    user = gateway.session(cookie, signed_session)
    if not user:
      return self.reply(401, {"success": False, "error": "Sign in required"})

    file_match = FILE.fullmatch(path)
    update_match = UPDATE.fullmatch(path)
    token = (file_match or update_match)
    is_get = self.command == "GET"
    allowed = ((UPLOAD.fullmatch(path) or update_match) and self.command == "POST"
               or file_match and (("/file/" in path and self.command in ("GET", "DELETE"))
                                  or "/file/" not in path and self.command == "POST")
               or is_get and path in SESSION_LISTS
               or is_get and path in ("/v1/media_files/list/user/" + user.get("username", ""),
                                       "/v1/media_files/list_v2/user/" + user.get("username", "")))
    if not allowed:
      return self.reply(404, {"success": False})
    if token and not gateway.owned_file(token.group(1), cookie, user, signed_session):
      return self.reply(404, {"success": False})
    if path == "/v1/media_files/batch":
      tokens = parse_qs(urlsplit(self.path).query).get("tokens", [])
      if not tokens or len(tokens) > 100:
        return self.reply(400, {"success": False})
      for media_token in tokens:
        if not re.fullmatch(TOKEN, media_token) or not gateway.owned_file(media_token, cookie, user, signed_session):
          return self.reply(404, {"success": False})

    body = None
    if self.command in ("POST", "DELETE"):
      raw_length = self.headers.get("Content-Length", "")
      if not raw_length.isdecimal() or self.headers.get("Transfer-Encoding"):
        return self.reply(411, {"success": False})
      length = int(raw_length)
      if length > MAX_UPLOAD:
        return self.reply(413, {"success": False, "error": "Maximum upload size is 64 MiB"})
      body = self.rfile.read(length)
      if len(body) != length:
        return self.reply(400, {"success": False})
      if "/cover_image/" in path:
        try:
          cover_token = json.loads(body)["cover_image_media_file_token"]
        except (ValueError, KeyError, TypeError):
          return self.reply(400, {"success": False})
        if not isinstance(cover_token, str) or not re.fullmatch(TOKEN, cover_token) or not gateway.owned_file(cover_token, cookie, user, signed_session):
          return self.reply(404, {"success": False})

    upstream_path = self.path
    if path == "/v1/media_files/list":
      # The legacy global list is public. Always replace with the user's SQL list.
      upstream_path = "/v1/media_files/list/user/" + quote(user["username"], safe="")
      if urlsplit(self.path).query:
        upstream_path += "?" + urlsplit(self.path).query
    status, data = gateway.upstream(self.command, upstream_path, cookie, body,
                                    self.headers.get("Content-Type", "application/json"), signed_session=signed_session)
    if 200 <= status < 300:
      data = gateway.rewrite(data)
    self.reply(status, data)

  def serve_media(self):
    if self.command not in ("GET", "HEAD"):
      return self.reply(405, {"success": False})
    gateway = self.server.gateway
    origin = self.headers.get("Origin")
    if origin and origin != gateway.origin:
      return self.reply(403, {"success": False})
    try:
      parsed = urlsplit(self.path)
      encoded, _, filename = parsed.path.removeprefix("/staging-media/").partition("/")
      path = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)).decode()
      query = parse_qs(parsed.query)
      expires = int(query["expires"][0])
      signature = query["signature"][0]
      valid = (safe_storage_path(path) and (not filename or unquote(filename) == path.rsplit("/", 1)[1])
               and int(time.time()) < expires <= int(time.time()) + LINK_TTL
               and hmac.compare_digest(signature, gateway.sign_path(path, expires)))
    except (ValueError, KeyError, UnicodeError):
      valid = False
    if not valid:
      return self.reply(403, {"success": False, "error": "Expired or invalid media link"})
    conn = http.client.HTTPConnection(gateway.filer, timeout=120)
    try:
      headers = {}
      if self.headers.get("Range"):
        headers["Range"] = self.headers["Range"]
      conn.request(self.command, "/buckets/artcraft-public" + quote(path, safe="/"), headers=headers)
      response = conn.getresponse()
      self.send_response(response.status)
      self.common_headers()
      for key in ("Content-Type", "Content-Length", "Content-Range", "Accept-Ranges", "Last-Modified"):
        if response.getheader(key):
          self.send_header(key, response.getheader(key))
      self.send_header("Connection", "close")
      self.end_headers()
      self.close_connection = True
      if self.command != "HEAD":
        while chunk := response.read(65536):
          self.wfile.write(chunk)
    finally:
      conn.close()


def main():
  logging.basicConfig(level=logging.INFO)
  gateway = Gateway(os.environ.get("MEDIA_API_UPSTREAM", "storyteller-web:12345"),
                    os.environ.get("MEDIA_FILER_UPSTREAM", "seaweedfs:8888"),
                    os.environ["ARTCRAFT_WEBAPP_EXTRA_ORIGIN"],
                    os.environ["MEDIA_PUBLIC_ORIGIN"], os.environ["MEDIA_LINK_SECRET"].encode())
  server = ThreadingHTTPServer(("0.0.0.0", 8080), Handler)
  server.gateway = gateway
  server.serve_forever()


if __name__ == "__main__":
  main()
