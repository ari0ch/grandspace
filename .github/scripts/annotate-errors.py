"""Turn rustc/clippy errors in a log file into GitHub annotations.

Annotations show up on the PR and through the API, so a failing build can
be diagnosed without downloading the full log.
"""
import re
import sys

text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
blocks = re.split(r"\n(?=(?:error|warning)(?:\[[A-Za-z0-9_:]+\])?: )", text)
shown = 0
for block in blocks:
    if not block.startswith("error") or shown >= 10:
        continue
    msg = block.strip()[:4000]
    msg = msg.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::error title=rustc::{msg}")
    shown += 1
if shown == 0:
    tail = text.strip()[-4000:]
    print("::error title=build log tail::" + tail.replace("%", "%25").replace("\n", "%0A"))
