#!/usr/bin/env python3
"""Keep a release's latest.json in step with the repacked Linux AppImage, and
check that every download it lists verifies against the updater key.

  updater-manifest.py patch  <latest.json> <AppImage.sig>
  updater-manifest.py verify <tauri.conf.json> <latest.json> <version> <download dir>

`patch` writes the signature from <AppImage.sig> into every latest.json entry
whose URL is that AppImage. `verify` downloads every URL in latest.json and
checks its signature the way the Tauri updater does (minisign, Ed25519, with
the pubkey from tauri.conf.json). It exits non-zero if any entry fails, if an
expected platform is missing, or if latest.json is for another version.

Used by the updater-manifest job in .github/workflows/release.yml.
Needs the `cryptography` package for `verify`.
"""
import base64
import hashlib
import json
import os
import sys
import urllib.request

EXPECTED_PLATFORMS = {
    "windows-x86_64",
    "windows-x86_64-nsis",
    "linux-x86_64",
    "linux-x86_64-appimage",
    "linux-x86_64-deb",
    "linux-x86_64-rpm",
}


def patch(latest_path: str, sig_path: str) -> None:
    with open(latest_path) as f:
        manifest = json.load(f)
    with open(sig_path) as f:
        signature = f.read().strip()
    # The .sig asset is named after the AppImage asset, e.g.
    # Drop.Desktop.Client_6.1.5_amd64.AppImage.sig
    appimage = os.path.basename(sig_path).removesuffix(".sig")
    entries = [
        name
        for name, entry in manifest["platforms"].items()
        if entry["url"].rsplit("/", 1)[-1] == appimage
    ]
    if not entries:
        sys.exit(f"no latest.json entry points at {appimage}")
    for name in entries:
        manifest["platforms"][name]["signature"] = signature
    with open(latest_path, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    print(f"latest.json: new signature for {', '.join(sorted(entries))}")


def _public_key(conf_path: str):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

    with open(conf_path) as f:
        conf = json.load(f)
    text = base64.b64decode(conf["plugins"]["updater"]["pubkey"]).decode()
    raw = base64.b64decode(text.splitlines()[1])
    return raw[2:10], Ed25519PublicKey.from_public_bytes(raw[10:42])


def _download(url: str, dest: str) -> tuple[bytes, bytes]:
    """Save url to dest; return (blake2b-512 digest, sha256 hex) of the bytes."""
    blake, sha = hashlib.blake2b(digest_size=64), hashlib.sha256()
    with urllib.request.urlopen(url, timeout=300) as resp, open(dest, "wb") as out:
        while chunk := resp.read(1 << 20):
            blake.update(chunk)
            sha.update(chunk)
            out.write(chunk)
    return blake.digest(), sha.hexdigest().encode()


def _check(signature_b64: str, path: str, prehash: bytes, key_id: bytes, key) -> str | None:
    """Return None if the signature is good, otherwise the reason it isn't."""
    from cryptography.exceptions import InvalidSignature

    lines = base64.b64decode(signature_b64).decode().splitlines()
    raw = base64.b64decode(lines[1])
    alg, sig_key_id, sig = raw[:2], raw[2:10], raw[10:74]
    if sig_key_id != key_id:
        return "signed with a different key"
    if alg == b"ED":
        message = prehash
    else:
        with open(path, "rb") as f:
            message = f.read()
    try:
        key.verify(sig, message)
    except InvalidSignature:
        return "signature does not match the file"
    trusted = lines[2].removeprefix("trusted comment: ").encode()
    try:
        key.verify(base64.b64decode(lines[3]), sig + trusted)
    except InvalidSignature:
        return "trusted comment signature does not match"
    return None


def verify(conf_path: str, latest_path: str, version: str, outdir: str) -> None:
    key_id, key = _public_key(conf_path)
    with open(latest_path) as f:
        manifest = json.load(f)
    problems = []
    if manifest.get("version") != version:
        problems.append(f"latest.json is for {manifest.get('version')}, expected {version}")
    missing = EXPECTED_PLATFORMS - manifest["platforms"].keys()
    if missing:
        problems.append(f"missing platforms: {', '.join(sorted(missing))}")

    os.makedirs(outdir, exist_ok=True)
    downloaded: dict[str, tuple[str, bytes]] = {}
    for name, entry in sorted(manifest["platforms"].items()):
        url = entry["url"]
        if url not in downloaded:
            dest = os.path.join(outdir, url.rsplit("/", 1)[-1])
            prehash, sha = _download(url, dest)
            downloaded[url] = (dest, prehash)
            print(f"downloaded {url.rsplit('/', 1)[-1]} sha256={sha.decode()}")
        dest, prehash = downloaded[url]
        reason = _check(entry["signature"], dest, prehash, key_id, key)
        if reason:
            problems.append(f"{name}: {reason} ({url})")
        else:
            print(f"{name}: signature ok")

    if problems:
        print("\nThe updater would reject or miss this release:", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        sys.exit(1)
    print(f"\nAll {len(manifest['platforms'])} latest.json entries verify for {version}.")


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "patch":
        patch(sys.argv[2], sys.argv[3])
    elif len(sys.argv) == 6 and sys.argv[1] == "verify":
        verify(*sys.argv[2:6])
    else:
        sys.exit(__doc__)
