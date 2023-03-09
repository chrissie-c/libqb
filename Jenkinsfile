pipeline {
    agent none

    stages {
        stage('Build Debian') {
	       agent {
	       	     label "debian"
	       }
            steps {
		echo 'Building..'
                sh 'sh autogen.sh'
		sh './configure'
		sh 'make'
		sh 'make check'
		sh 'make distcheck'
            }
        }
        stage('Build Fedora') {
	       agent {
	       	     label "fedora"
	       }
            steps {
		echo 'Building..'
                sh 'sh autogen.sh'
		sh './configure'
		sh 'make'
		sh 'make check'
		sh 'make distcheck'
            }
        }
    }
}