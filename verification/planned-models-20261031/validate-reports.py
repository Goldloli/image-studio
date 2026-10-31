#!/usr/bin/env python3
"""Read-only checks for the retained acceptance matrix and evidence bytes."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
repo = root.parent.parent


def load(name):
    return json.loads((root / name).read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


expected = {
    'K-T': 'I01 I02 I05 I06 I07 I08 I09 B01 B04 B10 P01 P02 P03 P04 P05 P07 P08 P09 P10 S03 S07 U01 U03 U05 U06',
    'K-E': 'I01 I02 I05 I06 I07 I08 I09 B02 B03 B04 B10 P01 P02 P03 P04 P05 P07 P08 P09 P10 S01 S02 S03 S07 U01 U02 U03 U05 U06',
    'B-R': 'I01 I03 I05 I06 I07 I08 I09 B05 B06 B07 B10 P01 P02 P03 P04 P06 P07 P08 P09 P10 S01 S02 S03 S04 S05 S07 U01 U02 U03 U04 U06',
    'R-U': 'I01 I04 I05 I06 I07 I08 I09 B08 B09 B10 P01 P02 P03 P04 P05 P07 P08 P09 P10 S01 S02 S03 S06 S07 U01 U02 U03 U05 U06',
    'batch': 'R01 R02 R03 R04',
}
expected_pairs = {(config, case) for config, ids in expected.items() for case in ids.split()}
report = load('batch-acceptance.json')
actual_pairs = [(case['configurationKey'], case['caseId']) for case in report['cases']]
assert len(actual_pairs) == len(set(actual_pairs)) == 118
assert set(actual_pairs) == expected_pairs
assert report['gateStatus'] == 'passed'
assert report['summary'] == dict(passed=118, failed=0, not_run=0, blocked=0)
allowed_kinds = {'metadata', 'source_review', 'fixture', 'device_probe', 'upstream_baseline', 'product_execution', 'native_daily'}
for case in report['cases']:
    assert case['schemaVersion'] == 1 and case['runId'] == 'planned-models-20261031'
    assert case['required'] is True and case['status'] == 'passed'
    assert case['evidenceKind'] in allowed_kinds and case['reason'] and case['evidencePaths']

fingerprints = load('batch-report-schema-validation.json')['evidenceSha256']
for name, expected_hash in fingerprints.items():
    path = root / name
    if not path.is_file():
        path = repo / name
    assert path.is_file() and digest(path) == expected_hash, name

image_count = 0
for sample in load('B08-realesrgan-quality.json')['samples']:
    assert digest(root / sample['source']) == sample['sourceSha256']
    assert digest(root / sample['output']) == sample['outputSha256']
    image_count += 2
seams = load('B09-realesrgan-tiles.json')
for record in [seams['formalT128'], seams['referenceLargeTile']]:
    assert digest(root / record['output']) == record['sha256']
    image_count += 1
for item in seams['fullSeamReview']:
    assert digest(root / item['path']) == item['sha256']
    image_count += 1
for item in load('native-exports-verified.json')['exports']:
    assert digest(root / item['path']) == item['sha256']
    image_count += 1
for item in load('z-regression/quality-review.json')['cases'].values():
    assert digest(root / 'z-regression' / item['output']) == item['sha256']
    image_count += 1
for name, expected_hash in load('realesrgan-run-identity.json')['manifests'].items():
    assert digest(repo / name) == expected_hash

print(json.dumps(dict(status='passed', requiredResults=118,
                      directEvidenceFiles=len(fingerprints), imageHashes=image_count,
                      pinnedRealesrganManifests=2), ensure_ascii=False))
