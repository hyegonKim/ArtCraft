#!/usr/bin/env python3
"""
Database status polling script. Runs a count query in a loop and logs results.
Reads MySQL credentials from the same secrets.toml as migrate.py.

Requirements:
  pip install pymysql
"""

import sys
import time
import tomllib
from datetime import datetime
from pathlib import Path
from urllib.parse import urlparse

try:
  import pymysql
except ImportError:
  print("pymysql is required: pip install pymysql")
  sys.exit(1)


SECRETS_PATH = "secrets.toml"

# -- Count query to poll --
#   QUERY = """
#     select count(*)
#     from media_files
#     where maybe_creator_user_token is null
#   """

QUERY = """
  select count(*)
  from media_files
  where media_type in ("audio", "wav")
    and created_at < NOW() - INTERVAL 6 MONTH
"""

POLL_INTERVAL_SECONDS = 60
FAILURE_BACKOFF_SECONDS = 5
MAX_BACKOFF_SECONDS = 300


def load_config():
  with open(SECRETS_PATH, "rb") as f:
    config = tomllib.load(f)
  url = config["database"]["url"]
  parsed = urlparse(url)
  return {
    "host": parsed.hostname,
    "port": parsed.port or 3306,
    "user": parsed.username,
    "password": parsed.password,
    "database": parsed.path.lstrip("/"),
  }


def connect(config):
  return pymysql.connect(
    host=config["host"],
    port=config["port"],
    user=config["user"],
    password=config["password"],
    database=config["database"],
    connect_timeout=10,
    read_timeout=30,
    write_timeout=30,
    autocommit=True,
  )


def run_status():
  config = load_config()
  conn = connect(config)
  failures = 0

  print(f"Connected to {config['host']}/{config['database']}")
  print(f"Polling every {POLL_INTERVAL_SECONDS}s with:\n{QUERY.strip()}\n")

  try:
    while True:
      try:
        query_start = time.monotonic()
        with conn.cursor() as cursor:
          cursor.execute(QUERY)
          count = cursor.fetchone()[0]

        failures = 0
        query_time = int(time.monotonic() - query_start)
        now = datetime.now().strftime("%H:%M:%S")
        print(f"  Remaining: {count:,} rows | Query: {query_time}s | Clock: {now}")

        time.sleep(POLL_INTERVAL_SECONDS)

      except (pymysql.OperationalError, pymysql.InterfaceError) as e:
        failures += 1
        backoff = min(FAILURE_BACKOFF_SECONDS * failures, MAX_BACKOFF_SECONDS)
        print(f"  Error ({failures}): {e}")
        print(f"  Reconnecting in {backoff}s...")
        time.sleep(backoff)
        try:
          conn.close()
        except Exception:
          pass
        conn = connect(config)

  except KeyboardInterrupt:
    print("\nInterrupted.")
  finally:
    conn.close()


if __name__ == "__main__":
  run_status()
