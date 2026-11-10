#!/usr/bin/env python3
"""Licence policy evaluation (scripts/release_licenses.py) against packaging/licenses.json."""
import json,pathlib,sys,unittest
HERE=pathlib.Path(__file__).resolve().parent;sys.path.insert(0,str(HERE))
import release_licenses as rl
POLICY=rl.Policy(json.loads((HERE.parent/'packaging/licenses.json').read_text()))
class Expressions(unittest.TestCase):
 def verdict(self,expression):
  # As collect() does: an expression that does not parse is rejected.
  try:return POLICY.evaluate(POLICY.normalize(expression))[0]
  except ValueError:return rl.REJECTED
 def test_permissive(self):
  for e in ['MIT','MIT/Apache-2.0','Apache-2.0 / MIT','(MIT OR Apache-2.0) AND Unicode-3.0','Apache-2.0 WITH LLVM-exception OR MIT','Apache 2.0','MPL-2.0','Apache-2.0 OR GPL-2.0-only']:
   with self.subTest(e):self.assertEqual(self.verdict(e),rl.ALLOWED)
 def test_copyleft_needs_review(self):
  for e in ['GPL-3.0-only','LGPL-2.1-or-later','MIT AND GPL-2.0+','AGPL-3.0','GPL-2.0 WITH Classpath-exception-2.0']:
   with self.subTest(e):self.assertEqual(self.verdict(e),rl.REVIEW)
 def test_unknown_or_missing_fails(self):
  for e in ['NOASSERTION','SEE LICENSE IN LICENSE.md','Proprietary','MIT AND (','MIT OR']:
   with self.subTest(e):self.assertEqual(self.verdict(e),rl.REJECTED)
 def test_malformed_raises(self):
  for e in ['MIT AND (Apache-2.0','MIT OR','AND MIT']:
   with self.subTest(e),self.assertRaises(ValueError):POLICY.evaluate(e)
 def test_problems_only_count_shipped(self):
  rows=[{'shipped':True,'status':'review'},{'shipped':False,'status':'rejected'},{'shipped':True,'status':'reviewed'}]
  self.assertEqual(rl.problems(rows),[rows[0]])
if __name__=='__main__':unittest.main()
