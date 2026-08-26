# Relay deployment identity

Canonical relay images from `ghcr.io/heap-cider/maju` carry two signed
attestations:

- SLSA build provenance maps the immutable image digest to the source commit
  and Docker workflow run.
- The Maju deployment-eligibility predicate records the successful same-SHA
  CI run and the exact Maju Helm chart version from that source commit.

The Docker workflow creates tagged multi-architecture manifests only after the
same full source SHA has a successful `CI` push run on `main` or `release`.
Architecture-specific build manifests may exist without tags while CI is
running or after it fails; they do not receive the deployment-eligibility
predicate and are not promotion inputs.

Verify a canonical eligible digest with:

```bash
gh attestation verify \
  oci://ghcr.io/heap-cider/maju@sha256:<digest> \
  --repo heap-cider/maju \
  --signer-workflow heap-cider/maju/.github/workflows/docker.yml \
  --predicate-type https://github.com/heap-cider/maju/attestations/deployment-eligibility/v1 \
  --source-digest <40-character-source-sha>
```

The predicate's `helm_chart.compatible_version` is image-to-chart metadata. It
does not describe database schema compatibility and does not relax Maju's rule
that migrations remain backwards compatible.

## Runtime inspection

The relay health listener exposes intrinsic build identity at `/_status`:

```json
{
  "service": "maju-relay",
  "version": "0.2.1",
  "uptime_seconds": 123,
  "build": {
    "source_sha": "<40-character-source-sha>",
    "id": "github-actions:<run-id>:<attempt>",
    "url": "https://github.com/heap-cider/maju/actions/runs/<run-id>/attempts/<attempt>"
  }
}
```

Non-CI builds report stable `unknown` or `local` fallback values instead of
claiming provenance they do not have.

## Helm digest pinning

Maju chart `0.1.8` and newer accept an immutable image digest:

```yaml
image:
  repository: ghcr.io/heap-cider/maju
  digest: sha256:<64-lowercase-hex-characters>
```

When `image.digest` is set, the chart renders `repository@digest` and ignores
`image.tag`. Existing tag-only values remain backwards compatible.
