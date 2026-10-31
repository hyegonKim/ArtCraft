#!/bin/bash
# ONLY FOR LOCAL DEV

USERNAME='storyteller'
PASSWORD='password'
HOST='localhost'

# --default-character-set=UTF8mb4 \

echo 'Inserting System User Roles...'
mysql -u "${USERNAME}" \
  -p${PASSWORD} \
  -h $HOST \
  -D storyteller \
  -e "source ./seed/sql/system_roles.sql"

echo 'Inserting Badges...'
mysql -u $USERNAME \
  -p${PASSWORD} \
  -h $HOST \
  -D storyteller \
  -e "source ./seed/sql/user_badges.sql"


