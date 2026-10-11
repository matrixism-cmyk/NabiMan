#!/usr/bin/env python3
"""Offline Ed25519 license issuer. Never install private keys on a NabiMan server."""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

DOMAIN = b'NabiMan license v1\0'
ID = re.compile(r'^[A-Za-z0-9_-]{1,64}$')
FEATURES = {'free': ['basic_monitoring'], 'pro': ['basic_monitoring', 'management', 'alerts', 'backups'],
            'enterprise': ['basic_monitoring', 'management', 'alerts', 'backups', 'ldap', 'oauth', 'api_keys', 'unlimited_servers']}


def openssl(*args, input_bytes=None):
    result = subprocess.run(['openssl', *args], input=input_bytes, capture_output=True, timeout=20)
    if result.returncode:
        raise ValueError('OpenSSL operation failed; check key type and input files')
    return result.stdout


def exclusive(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def no_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate JSON field')
        out[key] = value
    return out


def read_json(path, limit):
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('Input file exceeds size limit')
    return json.loads(data, object_pairs_hook=no_duplicates)


def public_hex(private):
    der = openssl('pkey', '-pubout', '-outform', 'DER', input_bytes=private)
    prefix = bytes.fromhex('302a300506032b6570032100')
    if len(der) != 44 or not der.startswith(prefix):
        raise ValueError('Private key is not Ed25519')
    return der[-32:].hex()


def generate(args):
    if args.private_key.exists() or args.private_key.is_symlink():
        raise ValueError('Private key already exists; choose a new offline path')
    if args.private_key.resolve() == args.public_keys.resolve():
        raise ValueError('Private and public key paths must differ')
    roots = read_json(args.public_keys, 65536) if args.public_keys.exists() else {'keys': []}
    if not isinstance(roots, dict) or set(roots) != {'keys'} or not isinstance(roots['keys'], list):
        raise ValueError('Invalid public key file')
    if any(not isinstance(k, dict) or set(k) != {'key_id', 'public_key'} or not isinstance(k['key_id'], str) or not ID.fullmatch(k['key_id']) or not isinstance(k['public_key'], str) or not re.fullmatch(r'[0-9a-fA-F]{64}', k['public_key']) for k in roots['keys']):
        raise ValueError('Invalid existing verification key')
    if len({k['key_id'] for k in roots['keys']}) != len(roots['keys']):
        raise ValueError('Duplicate existing key ID')
    if len(roots['keys']) >= 64 or any(k.get('key_id') == args.key_id for k in roots['keys']):
        raise ValueError('Key ID already present or key count limit reached')
    private = openssl('genpkey', '-algorithm', 'ED25519')
    roots['keys'].append({'key_id': args.key_id, 'public_key': public_hex(private)})
    exclusive(args.private_key, private)
    # Public rotation file replacement is atomic; existing private keys are never overwritten.
    args.public_keys.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(prefix='.license-roots-', dir=args.public_keys.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(roots, stream, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, args.public_keys)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)
    print('Created offline private key and public verification key file.')


def validate(claims):
    if not isinstance(claims, dict) or set(claims) != {'license_id', 'product', 'info'}:
        raise ValueError('Invalid claims fields')
    if claims['product'] != 'nabiman' or not isinstance(claims['license_id'], str) or not ID.fullmatch(claims['license_id']):
        raise ValueError('Invalid product or license ID')
    info = claims['info']
    if not isinstance(info, dict) or set(info) != {'tier', 'holder', 'issued_at', 'expires_at', 'max_servers', 'features'}:
        raise ValueError('Invalid license info fields')
    tier = info['tier']
    if tier not in FEATURES:
        raise ValueError('Unsupported tier')
    holder = info['holder']
    if not isinstance(holder, str) or not holder.strip() or len(holder.encode()) > 256 or any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in holder):
        raise ValueError('Invalid holder')
    dates = []
    for value in [info['issued_at'], info['expires_at']]:
        if value is None and dates:
            continue
        date = datetime.date.fromisoformat(value)
        if date.isoformat() != value:
            raise ValueError('Dates must use YYYY-MM-DD')
        dates.append(date)
    if len(dates) == 2 and dates[1] < dates[0]:
        raise ValueError('Expiry precedes issue date')
    features = info['features']
    if not isinstance(features, list) or not features or any(not isinstance(f, str) or f not in FEATURES[tier] for f in features) or len(set(features)) != len(features):
        raise ValueError('Invalid features for tier')
    limit = info['max_servers']
    if limit is not None and (type(limit) is not int or not 1 <= limit <= 4294967295):
        raise ValueError('Invalid server limit')
    if tier == 'free' and limit != 1 or tier == 'pro' and (limit is None or limit > 10):
        raise ValueError('Invalid server limit for tier')


def issue(args):
    if args.private_key.is_symlink() or not args.private_key.is_file():
        raise ValueError('Use a regular offline private key file')
    if args.private_key.stat().st_mode & 0o077:
        raise ValueError('Private key permissions must exclude group and others (chmod 600)')
    private = args.private_key.read_bytes()
    public_hex(private)  # Validate algorithm before signing.
    claims = read_json(args.claims, 8192)
    validate(claims)
    payload = json.dumps(claims, ensure_ascii=False, separators=(',', ':')).encode()
    if len(payload) > 8192:
        raise ValueError('Encoded claims exceed size limit')
    with tempfile.TemporaryDirectory(prefix='nabiman-offline-issuer-') as tmp:
        message = Path(tmp) / 'message'
        exclusive(message, DOMAIN + args.key_id.encode() + b'\0' + payload)
        signature = openssl('pkeyutl', '-sign', '-rawin', '-inkey', str(args.private_key), '-in', str(message))
    if len(signature) != 64:
        raise ValueError('Unexpected Ed25519 signature size')
    envelope = {'version': 1, 'key_id': args.key_id, 'payload': payload.hex(), 'signature': signature.hex()}
    token = 'NBL1.' + json.dumps(envelope, separators=(',', ':')).encode().hex()
    exclusive(args.output, (token + '\n').encode())
    print('Issued signed license file. No private key is included.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    generate_parser = sub.add_parser('generate-key')
    generate_parser.add_argument('--private-key', type=Path, required=True)
    generate_parser.add_argument('--public-keys', type=Path, required=True)
    issue_parser = sub.add_parser('issue')
    issue_parser.add_argument('--private-key', type=Path, required=True)
    issue_parser.add_argument('--claims', type=Path, required=True)
    issue_parser.add_argument('--output', type=Path, required=True)
    for command in [generate_parser, issue_parser]:
        command.add_argument('--key-id', required=True)
    args = parser.parse_args()
    try:
        if not ID.fullmatch(args.key_id):
            raise ValueError('Invalid key ID')
        (generate if args.command == 'generate-key' else issue)(args)
    except (OSError, ValueError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, 'Issuing failed: ' + str(error) + '\n')


if __name__ == '__main__':
    main()
