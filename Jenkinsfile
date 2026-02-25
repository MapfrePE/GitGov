pipeline {
  agent any

  environment {
    // Ajusta estas variables en Jenkins (Manage Jenkins -> System / Credentials)
    GITGOV_URL = credentials('gitgov-url') // ej: http://host.docker.internal:3000
    GITGOV_API_KEY = credentials('gitgov-api-key')
    // Opcional si activaste JENKINS_WEBHOOK_SECRET en el server
    GITGOV_JENKINS_SECRET = credentials('gitgov-jenkins-secret')
  }

  options {
    timestamps()
  }

  stages {
    stage('Checkout') {
      steps {
        checkout scm
      }
    }

    stage('Policy Check (Advisory)') {
      steps {
        script {
          def repoName = env.GIT_URL ? env.GIT_URL.replaceFirst('^.*github\\.com[/:]', '').replaceFirst('\\.git$', '') : ''
          def branchName = env.BRANCH_NAME ?: env.GIT_BRANCH ?: 'unknown'
          def commitSha = env.GIT_COMMIT ?: sh(script: 'git rev-parse HEAD', returnStdout: true).trim()

          sh """
            curl -sS -X POST ${GITGOV_URL}/policy/check \
              -H "Authorization: Bearer ${GITGOV_API_KEY}" \
              -H "Content-Type: application/json" \
              -d '{
                "repo":"${repoName}",
                "branch":"${branchName}",
                "commit":"${commitSha}"
              }' || true
          """
        }
      }
    }

    stage('Build') {
      steps {
        echo 'Reemplaza este stage con tu build real'
      }
    }
  }

  post {
    success {
      script {
        notifyGitGov('success')
      }
    }
    failure {
      script {
        notifyGitGov('failure')
      }
    }
    unstable {
      script {
        notifyGitGov('unstable')
      }
    }
    aborted {
      script {
        notifyGitGov('aborted')
      }
    }
  }
}

def notifyGitGov(String status) {
  def repoName = env.GIT_URL ? env.GIT_URL.replaceFirst('^.*github\\.com[/:]', '').replaceFirst('\\.git$', '') : ''
  def branchName = env.BRANCH_NAME ?: env.GIT_BRANCH ?: 'unknown'
  def commitSha = env.GIT_COMMIT ?: sh(script: 'git rev-parse HEAD', returnStdout: true).trim()
  def durationMs = currentBuild.duration ?: 0
  def payload = """{
    "pipeline_id":"${env.JOB_NAME ?: 'unknown'}#${env.BUILD_NUMBER ?: '0'}",
    "job_name":"${env.JOB_NAME ?: 'unknown'}",
    "status":"${status}",
    "commit_sha":"${commitSha}",
    "branch":"${branchName}",
    "repo_full_name":"${repoName}",
    "duration_ms":${durationMs},
    "triggered_by":"${env.BUILD_USER_ID ?: env.BUILD_USER ?: 'jenkins'}",
    "stages":[],
    "artifacts":[],
    "timestamp":${System.currentTimeMillis()}
  }"""

  def secretHeader = env.GITGOV_JENKINS_SECRET?.trim()
    ? "-H \\\"x-gitgov-jenkins-secret: ${env.GITGOV_JENKINS_SECRET}\\\""
    : ""

  sh """
    curl -sS -X POST ${env.GITGOV_URL}/integrations/jenkins \
      -H "Authorization: Bearer ${env.GITGOV_API_KEY}" \
      -H "Content-Type: application/json" \
      ${secretHeader} \
      -d '${payload}'
  """
}
