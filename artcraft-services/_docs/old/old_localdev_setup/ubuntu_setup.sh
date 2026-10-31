#!/bin/bash

echo 'Installing APT packages...'

# OpenSSL requirements: libssl-dev, pkg-config
# Database requirements: mysql-server, libmysqlclient-dev
# Other tools: jq (db codegen)
sudo apt-get install -y \
  jq \
  libmysqlclient-dev \
  libssl-dev \
  mysql-server \
  pkg-config \
  --no-install-recommends

echo 'Installing Cargo packages...'

# Install Diesel CLI for migrations
cargo install diesel_cli \
  --no-default-features \
  --features mysql

# Install SQLx for codegen
cargo install sqlx-cli \
  --no-default-features \
  --features rustls,mysql

echo 'Installing Database user...'

# Create DB account (using default system mysql password)
sudo mysql \
  -u "root" \
  -p"password" \
  -e "use mysql; CREATE DATABASE IF NOT EXISTS storyteller; CREATE USER IF NOT EXISTS 'storyteller'@'localhost' IDENTIFIED BY 'password'; GRANT ALL PRIVILEGES ON storyteller.* TO 'storyteller'@'localhost';"


