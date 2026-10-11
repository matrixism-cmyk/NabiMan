#!/usr/bin/env python3
"""Verify the shipped normal dependency graph, separately from test-only h2."""
import json,pathlib,subprocess
root=pathlib.Path(__file__).resolve().parents[1]
result=subprocess.run(['cargo','tree','--locked','--edges','normal','--prefix','none','--format','{p}'],cwd=root/'backend',capture_output=True,text=True,check=True)
rows={line.removesuffix(' (*)') for line in result.stdout.splitlines()}
assert not any(row.startswith('h2 ') for row in rows),'Backend HTTP/2 dependency was re-enabled; review the protocol boundary'
assert 'actix-http v3.18.12' in rows,'Review HTTP framing/WebSocket security fixes before changing the pinned dependency baseline'
print(json.dumps({'passed':True,'normal_graph_has_h2':False,'actix_http':'3.18.12','limits':'Test-only h2 remains in lockfile and must still be reported by cargo audit'}))
