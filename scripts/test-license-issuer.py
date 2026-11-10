#!/usr/bin/env python3
"""The offline issuer: older licences stay byte-for-byte the same, plans are
issued with their limits, and plans the server would refuse are refused here."""
import json
from pathlib import Path
import subprocess
import tempfile

ISSUER = Path(__file__).resolve().parents[1] / 'tools/license-issuer.py'
DOMAIN = b'NabiMan license v1\0'


def run(*args):
    return subprocess.run(['python3', str(ISSUER), *map(str, args)], capture_output=True, timeout=30)


def base():
    return {'license_id': 'customer-001', 'product': 'nabiman', 'info': {
        'tier': 'pro', 'holder': 'Example customer', 'issued_at': '2026-11-01', 'expires_at': '2027-11-01',
        'max_servers': 10, 'features': ['basic_monitoring', 'management']}}


def planned(**changes):
    claims = base()
    plan = {'plan': 'team', 'deployment': 'installed', 'max_users': 20, 'max_servers': 10,
            'features': ['orchestration', 'recording'], 'grace_days': 14}
    plan.update(changes)
    claims['info']['limits'] = {k: v for k, v in plan.items() if v is not ...}
    return claims


with tempfile.TemporaryDirectory(prefix='nabiman-issuer-test-') as tmp:
    d = Path(tmp)
    private, roots = d / 'issuer.pem', d / 'roots.json'
    assert run('generate-key', '--key-id', 'test-issuer', '--private-key', private, '--public-keys', roots).returncode == 0
    public = bytes.fromhex(json.loads(roots.read_text())['keys'][0]['public_key'])
    der = d / 'public.der'
    der.write_bytes(bytes.fromhex('302a300506032b6570032100') + public)
    issued = 0

    def issue(claims):
        global issued
        issued += 1
        source, output = d / f'claims-{issued}.json', d / f'licence-{issued}.key'
        source.write_text(json.dumps(claims))
        result = run('issue', '--key-id', 'test-issuer', '--private-key', private, '--claims', source, '--output', output)
        if result.returncode:
            assert not output.exists(), 'a refused licence leaves no file'
            return None
        envelope = json.loads(bytes.fromhex(output.read_text().strip().removeprefix('NBL1.')))
        payload = bytes.fromhex(envelope['payload'])
        message, signature = d / f'message-{issued}', d / f'signature-{issued}'
        message.write_bytes(DOMAIN + b'test-issuer\0' + payload)
        signature.write_bytes(bytes.fromhex(envelope['signature']))
        check = subprocess.run(['openssl', 'pkeyutl', '-verify', '-pubin', '-keyform', 'DER', '-inkey', str(der),
                                '-rawin', '-in', str(message), '-sigfile', str(signature)], capture_output=True, timeout=20)
        assert check.returncode == 0, 'signature verifies with the published key'
        return payload

    # The older format: exactly the claims given, no plan field added.
    old = issue(base())
    assert old == json.dumps(base(), ensure_ascii=False, separators=(',', ':')).encode()
    assert b'limits' not in old
    # A plan travels inside info, signed with the rest.
    plan = json.loads(issue(planned()))
    assert plan['info']['limits']['max_users'] == 20 and plan['info']['limits']['grace_days'] == 14
    # A plan sets its own server limit (the tier's fixed ones do not apply) and may
    # name features this issuer does not know yet.
    roomy = planned(max_servers=500, features=['recording', 'future_feature'])
    roomy['info']['max_servers'] = 500
    assert issue(roomy) is not None
    minimal = base()
    minimal['info']['limits'] = {'plan': 'hosted-remote', 'max_servers': 10}
    assert issue(minimal) is not None
    for bad in [planned(plan='Team!'), planned(plan=''), planned(deployment='cloud'), planned(max_users=0),
                planned(max_users=-1), planned(grace_days=91), planned(max_servers=11),
                planned(features=['recording', 'recording']), planned(features=['Bad Feature']),
                planned(max_cpus=4), planned(plan=...)]:
        assert issue(bad) is None, bad['info']['limits']
    # Without a plan the tier's rules still hold.
    wide = base()
    wide['info']['max_servers'] = 11
    assert issue(wide) is None
print('license issuer: ok')
