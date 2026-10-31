#!/usr/bin/env python3
"""
Database migration script for running queries in a loop with failure fallback.
Reads MySQL credentials from a TOML secrets file.

Requirements:
  pip install pymysql

Secrets file (script/database/secrets.toml):
  [database]
  url = "mysql://user:password@host:3306/dbname"
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

# -- Migration query to run in batches --
# Tweak this as needed. Use LIMIT to control batch size.
#  QUERY = """
#    delete
#    from media_files
#    where maybe_creator_user_token is null
#    limit 10000
#  """

QUERY = """
  delete from media_files 
  where media_type in ("audio", "wav") 
  and created_at < NOW() - INTERVAL 6 MONTH
  limit 10000  
"""

BATCH_PAUSE_SECONDS = 0.5
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


def run_migration():
  config = load_config()
  conn = connect(config)
  total_affected = 0
  failures = 0
  start_time = time.monotonic()

  print(f"Connected to {config['host']}/{config['database']}")
  print(f"Running query:\n{QUERY.strip()}\n")

  try:
    while True:
      try:
        batch_start = time.monotonic()
        with conn.cursor() as cursor:
          cursor.execute(QUERY)
          affected = cursor.rowcount

        total_affected += affected
        failures = 0
        batch_time = int(time.monotonic() - batch_start)
        elapsed = int(time.monotonic() - start_time)
        now = datetime.now().strftime("%H:%M:%S")
        print(f"  Batch: {affected} rows {batch_time}s | Total: {total_affected} rows {elapsed}s | Clock: {now}")

        if affected == 0:
          print("No more rows to process. Done.")
          break

        time.sleep(BATCH_PAUSE_SECONDS)

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
    print(f"\nInterrupted. Total rows affected: {total_affected}")
  finally:
    conn.close()

  print(f"Finished. Total rows affected: {total_affected}")


if __name__ == "__main__":
  run_migration()
