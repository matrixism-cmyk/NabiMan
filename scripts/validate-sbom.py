#!/usr/bin/env python3
"""Validate CycloneDX against hash-pinned official schemas; only schemas are downloaded."""
import hashlib,json,pathlib,sys,urllib.request
import jsonschema
from referencing import Registry,Resource
HASHES={'bom-1.6.schema.json':'3e92dddbc30cf7f6a02b80f0942b1a4cfd4fb1c26f1dfc4310afa9d613cafb93','spdx.schema.json':'baa9d3bd1ed57b6751b0887edead6b5063ff53ff7429cf85d476c6c94af0166e','jsf-0.82.schema.json':'8bae002c25e723db7ee1f26afde680ae1a2b1a8f6b4b4b0fd65dc3becb090aae'}
resources=[];schemas={}
for name,digest in HASHES.items():
 with urllib.request.urlopen('https://raw.githubusercontent.com/CycloneDX/specification/1.6/schema/'+name,timeout=30) as response:raw=response.read(4*1024*1024)
 if hashlib.sha256(raw).hexdigest()!=digest:raise SystemExit('Official schema digest mismatch')
 schemas[name]=json.loads(raw)
 for scheme in ['http','https']:resources.append((scheme+'://cyclonedx.org/schema/'+name,Resource.from_contents(schemas[name])))
value=json.loads(pathlib.Path(sys.argv[1]).read_text())
jsonschema.Draft7Validator(schemas['bom-1.6.schema.json'],registry=Registry().with_resources(resources),format_checker=jsonschema.FormatChecker()).validate(value)
print(json.dumps({'cyclonedx_schema':'1.6','schema_validation':True,'components':len(value['components']),'schema_sha256':HASHES['bom-1.6.schema.json']}))
