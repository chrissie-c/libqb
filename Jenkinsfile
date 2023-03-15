@Library('CCtestLib') _

pipeline {
    agent none
    stages {
        stage('Build and Test') {
            matrix {
                agent {
                    label "${PLATFORM}"
                }
                axes {
                    axis {
                        name 'PLATFORM'
                        values 'fedora', 'debian'
                    }
                }
                stages {
                    stage('Build & Test') {
                        steps {
			    runstuff(project:"CCTest", branch:"main")
			}
                    }
                }
                post {
                    always {
		        archiveArtifacts artifacts: 'tests/test-suite.log', fingerprint: true
                    }
                }
            }
        }
    }
}
