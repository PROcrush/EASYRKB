#!/bin/zsh
set -euo pipefail

project_dir="${0:A:h}"
app_binary="$project_dir/../downloads/EASYRKB.app/Contents/MacOS/rbxport"
if [[ ! -x "$app_binary" ]]; then
  print '未找到 EASYRKB 应用，请先按 DEVELOPMENT.zh-CN.md 构建。'
  exit 1
fi

exec env -u RBXPORT_OPTIONS -u RBXPORT_TEST -u RBXPORT_TEST_PORT -u RBXPORT_TEST_HEADLESS "$app_binary"
