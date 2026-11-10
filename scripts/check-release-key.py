#!/usr/bin/env python3
"""Release signing consistency checks (run in CI and before every release).

1. docs/release/nabiman-release.pub is a minisign public key (or the placeholder,
   with --allow-placeholder: CI before a key exists; releases never pass it).
2. install.sh embeds exactly that key, so an installer verifies packages with the
   repository's key and never with one downloaded alongside the package.
3. The minisign verifier embedded in install.sh is byte-identical to the one in
   scripts/verify-release.py.
"""
import argparse,importlib.util,pathlib,re,sys
ROOT=pathlib.Path(__file__).resolve().parent.parent
BEGIN='# --- minisign verifier begin';END='# --- minisign verifier end ---'
def block(text,name):
 start=text.find(BEGIN);end=text.find(END)
 if start<0 or end<start:raise SystemExit(name+': minisign verifier markers not found')
 return text[start:end]
def main():
 parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter);parser.add_argument('--allow-placeholder',action='store_true');a=parser.parse_args()
 install=(ROOT/'install.sh').read_text();verifier=(ROOT/'scripts/verify-release.py').read_text()
 if block(install,'install.sh')!=block(verifier,'verify-release.py'):raise SystemExit('install.sh and verify-release.py carry different minisign verifiers')
 lines=[l.strip() for l in (ROOT/'docs/release/nabiman-release.pub').read_text().splitlines() if l.strip() and not l.startswith('untrusted comment:')]
 if len(lines)!=1:raise SystemExit('docs/release/nabiman-release.pub must hold one key line')
 key=lines[0]
 embedded=re.findall(r"^NABIMAN_RELEASE_PUBKEY='([^']*)'$",install,re.M)
 if len(embedded)!=1:raise SystemExit('install.sh must set NABIMAN_RELEASE_PUBKEY exactly once')
 if embedded[0]!=key:raise SystemExit('install.sh embeds a different release key than docs/release/nabiman-release.pub')
 if key=='PLACEHOLDER':
  if not a.allow_placeholder:raise SystemExit('No release signing key yet: generate one (docs/release/README.md) before releasing')
  print('release key: placeholder (allowed for CI only)');return
 spec=importlib.util.spec_from_file_location('verify',ROOT/'scripts/verify-release.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
 key_id,_=v.minisign_public_key(key)
 print('release key OK: minisign key id '+key_id[::-1].hex().upper())
if __name__=='__main__':main()
