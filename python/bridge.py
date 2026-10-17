"""McHarness Python bridge — v0.0.1.

The research half: talks to the same model endpoints the Rust core uses,
so research tooling and the Ingestor's mining phase share one model layer.
"""

from __future__ import annotations

import json
import os
import urllib.request


OLLAMA_URL = os.environ.get("MH_MODEL_URL", "http://localhost:11434/v1/chat/completions")


def infer(model: str, prompt: str, user_input: str) -> str:
    body = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": user_input},
        ],
    }).encode()
    req = urllib.request.Request(
        OLLAMA_URL,
        data=body,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req) as resp:
        data = json.loads(resp.read())
    return data["choices"][0]["message"]["content"]


def record_lineage(lineage: str, source_url: str, status: str, crown_jewels: list[str]) -> dict:
    """Mirror of the Ingestor's record shape — Forge Ledger data in motion."""
    return {
        "lineage": lineage,
        "source_url": source_url,
        "status": status,
        "crown_jewels": crown_jewels,
    }


if __name__ == "__main__":
    print("McHarness bridge v0.0.1 — the bone is set. The blood is next.")
