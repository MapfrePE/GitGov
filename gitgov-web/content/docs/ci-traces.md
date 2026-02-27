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
GitGov integrates with Jenkins via a REST API call from your `Jenkinsfile`. After each build, a `curl` step posts the result to the Control Plane's `/integrations/jenkins` endpoint.
- **Automatic Metadata Injection**: Job name, commit SHA, branch, build duration, and stage results are captured.
- **Failure Analysis**: Correlates specific code changes with build regressions and failed stages.

### GitHub Webhooks
Connect your repositories to receive real-time push, pull request, and review events. This allows GitGov to verify that every pull request has been audited and approved according to your organization's policies.

---

## Establishing Evidence of Compliance

By using CI Traceability, you can generate automated reports for compliance audits:

- **Build Provenance**: Verification that a specific artifact was built from a specific commit on a specific server.
- **Approval Chains**: Evidence that the code was reviewed by an authorized peer before merging.
- **Environment State**: Real-time visibility into which commit is currently deployed in Dev, Staging, or Production.

---

## Configuration Example

Add a `post` step to your `Jenkinsfile` that calls the Control Plane REST API directly:

```groovy
pipeline {
    agent any
    environment {
        GITGOV_URL    = 'http://your-control-plane:3000'
        GITGOV_KEY    = credentials('gitgov-admin-api-key')
    }
    stages {
        stage('Build') {
            steps {
                // ... your existing build steps ...
            }
        }
    }
    post {
        always {
            script {
                def result = currentBuild.result ?: 'SUCCESS'
                def ts     = System.currentTimeMillis()
                sh """
                    curl -s -X POST \${GITGOV_URL}/integrations/jenkins \\
                      -H "Authorization: Bearer \${GITGOV_KEY}" \\
                      -H "Content-Type: application/json" \\
                      -d '{
                        "pipeline_id": "\${env.BUILD_TAG}",
                        "job_name": "\${env.JOB_NAME}",
                        "status": "\${result.toLowerCase()}",
                        "commit_sha": "\${env.GIT_COMMIT}",
                        "branch": "\${env.GIT_BRANCH}",
                        "repo_full_name": "YourOrg/YourRepo",
                        "duration_ms": \${currentBuild.duration},
                        "triggered_by": "\${env.BUILD_USER_ID ?: 'ci'}",
                        "timestamp": \${ts}
                      }'
                """
            }
        }
    }
}
```

Store the API key as a Jenkins credential (`gitgov-admin-api-key`) of type **Secret text**. The endpoint requires an admin-role Bearer token.

> [!IMPORTANT]
> **Integrity Guarantee**: Once a build is linked to a commit in GitGov, the record is locked. Any attempt to "re-tag" an existing build to a new commit will be flagged in the audit trail.

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
