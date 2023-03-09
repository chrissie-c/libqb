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
                            echo "Do Build and Test for ${PLATFORM}"
			    sh 'sh autogen.sh'
                            sh './configure'
                            sh 'make'
                            sh 'make check'
                            sh 'make distcheck'
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
