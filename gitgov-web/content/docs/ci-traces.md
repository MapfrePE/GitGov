---
title: CI/CD Traceability
description: Bridge the gap between source code, build artifacts, and production deployments via Jenkins, Jira, and GitHub integrations.
order: 5
---

A major blind spot in software security is the "phantom build" — code that exists in production but has no verifiable link back to a specific commit or developer. GitGov bridges this gap by integrating directly with your CI/CD pipelines and project management tools.

---

## The Traceability Chain

GitGov establishes an end-to-end link between your source code and your deployment environments:

1. **Commit Phase**: Metadata is captured by GitGov Desktop (`commit_sha`, `branch`, `author`, `timestamp`).
2. **Build Phase**: Your CI pipeline posts build results to the Control Plane. GitGov captures job name, build status, duration, and per-stage results.
3. **Correlation**: The Control Plane matches the commit SHA to the build ID, creating a verifiable chain of custody.

---

## Supported Integrations

### Jenkins (V1.2-A — Live)
GitGov integrates with Jenkins via a REST API call from your `Jenkinsfile`. After each build, a `curl` step posts the result to the Control Plane's `/integrations/jenkins` endpoint.

- **Captured metadata**: Job name, commit SHA, branch, build status, build duration, per-stage results, triggered-by user, and raw payload.
- **Correlation**: The Control Plane automatically matches `commit_sha` to existing commit events, creating a `CommitPipelineCorrelation` record.
- **Failure analysis**: Correlates specific code changes with build regressions and failed stages.

### Jira (V1.2-B — Preview)
GitGov integrates with Jira webhooks to capture ticket events and compute ticket coverage. The `/integrations/jira/ticket-coverage` endpoint reports the percentage of commits in a repository that are linked to a Jira ticket.

- **Coverage tracking**: Know what percentage of your commits reference a valid Jira ticket.
- **Batch correlation**: The `/integrations/jira/correlate` endpoint runs a bulk correlation pass against all recent commits.

### GitHub Webhooks
Connect your repositories to receive real-time push, pull request, and status events. This allows GitGov to verify that every pull request has been audited and approved according to your organization's policies.

---

## Jenkins Configuration Example

Add a `post` step to your `Jenkinsfile` that calls the Control Plane REST API:

```groovy
pipeline {
    agent any
    environment {
        GITGOV_URL = 'https://your-control-plane'
        GITGOV_KEY = credentials('gitgov-admin-api-key')
    }
    stages {
        stage('Build') {
            steps {
                // ... your existing build steps ...
            }
        }
        stage('Test') {
            steps {
                // ... your existing test steps ...
            }
        }
        stage('Deploy') {
            steps {
                // ... your existing deploy steps ...
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
                        "job_name":    "\${env.JOB_NAME}",
                        "status":      "\${result.toLowerCase()}",
                        "commit_sha":  "\${env.GIT_COMMIT}",
                        "branch":      "\${env.GIT_BRANCH}",
                        "repo_full_name": "YourOrg/YourRepo",
                        "duration_ms": \${currentBuild.duration},
                        "triggered_by": "\${env.BUILD_USER_ID ?: 'ci'}",
                        "timestamp":   \${ts},
                        "stages": [
                          {"name": "Build",  "status": "success", "duration_ms": 134000},
                          {"name": "Test",   "status": "success", "duration_ms": 272000},
                          {"name": "Deploy", "status": "success", "duration_ms": 63000}
                        ]
                      }'
                """
            }
        }
    }
}
```

Store the API key as a Jenkins credential (`gitgov-admin-api-key`) of type **Secret text**. The endpoint requires an admin-role Bearer token.

> [!IMPORTANT]
> **Integrity Guarantee**: Once a build is linked to a commit in GitGov, the record is locked and append-only. Any attempt to "re-tag" an existing build to a different commit will be logged in the audit trail.

---

## Policy Check Integration (Advisory)

Before executing a build, your Jenkins pipeline can query the Control Plane for a policy advisory:

```groovy
stage('Policy Check') {
    steps {
        script {
            def response = sh(
                script: """curl -s -X POST \${GITGOV_URL}/policy/check \\
                  -H "Authorization: Bearer \${GITGOV_KEY}" \\
                  -H "Content-Type: application/json" \\
                  -d '{"repo_name": "YourOrg/YourRepo", "commit_sha": "\${env.GIT_COMMIT}", "branch": "\${env.GIT_BRANCH}", "user_login": "\${env.GIT_AUTHOR_NAME}"}'""",
                returnStdout: true
            ).trim()
            echo "GitGov policy check: ${response}"
            // Parse response and optionally fail the build on violations
        }
    }
}
```

The response includes `allowed`, `reasons`, and `warnings`. This step is currently **advisory** — it logs compliance status but does not block the build unless you add explicit failure logic.

---

## Establishing Evidence of Compliance

By using CI Traceability, you can generate automated reports for compliance audits:

- **Build Provenance**: Verification that a specific artifact was built from a specific commit on a specific server.
- **Approval Chains**: Evidence that the code was reviewed and approved before merging.
- **Ticket Coverage**: The percentage of commits linked to a tracked Jira ticket (via `/integrations/jira/ticket-coverage`).
- **Audit Log Export**: Export full JSON reports via the `/export` endpoint for SOC 2 or ISO 27001 audits.

---

## Summary of Capabilities

| Feature | Endpoint | Status |
|---------|----------|--------|
| Link commits to Jenkins builds | `POST /integrations/jenkins` | Live (V1.2-A) |
| Commit–pipeline correlation query | `GET /integrations/jenkins/correlations` | Live (V1.2-A) |
| Pipeline health dashboard widget | `GET /integrations/jenkins/status` | Live (V1.2-A) |
| Jira ticket coverage | `GET /integrations/jira/ticket-coverage` | Preview (V1.2-B) |
| Jira batch correlation | `POST /integrations/jira/correlate` | Preview (V1.2-B) |
| GitHub webhook ingest | `POST /webhooks/github` | Live |
| Audit log export (JSON) | `POST /export` | Live |
| CI policy advisory check | `POST /policy/check` | Live (advisory) |

## End of Core Documentation

- [**Return to Home**](/)
- [**Contact Sales for Enterprise Support**](/contact)
