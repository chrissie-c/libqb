pipeline {
    agent any

    stages {
        stage('Build') {
            steps {
		echo 'Building..'
                sh 'sh autogen.sh'
		sh './configure'
		sh 'make'
            }
        }
        stage('Test') {
            steps {
                echo 'Testing..'
		sh 'make check'
		archiveArtifacts artifacts: 'tests/test-suite.log', fingerprint: true
            }
        }
        stage('Deploy') {
            steps {
                echo 'Deploying....'
		sh 'make distcheck'
            }
        }
    }
}