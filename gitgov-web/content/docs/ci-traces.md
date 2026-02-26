---
title: CI/CD Traceability
description: Bridge the gap between source code, build artifacts, and production deployments.
order: 5
---

A major blind spot in software security is the "phantom build" — code that exists in production but has no verifiable link back to a specific commit or developer. GitGov bridges this gap by integrating directly with your CI/CD pipelines.

---

## The Traceability Chain

GitGov establishes a bidirectional link between your source code and your deployment environments:

1. **Commit Phase**: Metadata is captured by GitGov Desktop.
2. **Build Phase**: GitGov's CI integration captures the Build ID, Environment, and Build Success status.
3. **Correlation**: The Control Plane matches the Commit Hash to the Build ID.

---

## Supported Integrations

### Jenkins Integration
GitGov provides a lightweight Jenkins plugin (and shared library) that automatically reports build events to the Control Plane.
- **Automatic Metadata Injection**: Injects the build URL and job name into the GitGov event stream.
- **Failure Analysis**: Correlates specific code changes with build regressions.

### GitHub Webhooks
Connect your repositories to receive real-time push and pull request events. This allows GitGov to verify that every pull request has been audited and approved according to your organization's policies.

---

## Establishing Evidence of Compliance

By using CI Traceability, you can generate automated reports for compliance audits:

- **Build Provenance**: Verification that a specific artifact was built from a specific commit on a specific server.
- **Approval Chains**: Evidence that the code was reviewed by an authorized peer before merging.
- **Environment State**: Real-time visibility into which commit is currently deployed in Dev, Staging, or Production.

---

## Configuration Highlight

To enable CI traceability in your `Jenkinsfile`, you simply add the GitGov wrapper:

```groovy
pipeline {
    agent any
    stages {
        stage('Audit') {
            steps {
                // Notifies GitGov that this build is starting for a specific commit
                gitgovNotify(status: 'STARTING', serverUrl: 'https://gitgov.internal')
            }
        }
        // ... build steps ...
    }
    post {
        always {
            gitgovNotify(status: currentBuild.result)
        }
    }
}
```

> [!IMPORTANT]
> **Integrity Guarantee**: Once a build is linked to a commit in GitGov, the record is locked. Any attempt to "re-tag" an existing build to a new commit will trigger a high-priority security alert.

---

## Summary of Capabilities

| Feature | Description |
|---------|-------------|
| **Link Commits to Builds** | Know exactly which build produced a specific artifact. |
| **Audit Log Exports** | Export full CSV/JSON reports for SOC2 or ISO audits. |
| **Drift Detection** | Identify if production code differs from the last governed build. |

## End of Core Documentation

- [**Return to Home**](/)
- [**Contact Sales for Enterprise Support**](/contact)
