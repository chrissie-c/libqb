pipeline {
    agent fedora37
    environment {
	when {
	    environment name: 'MODE', value: 'development'
	}
	CONFIG_FLAGS = ''
    }
    environment {
	when {
	    environment name: 'MODE', value: 'production'
	}
	CONFIG_FLAGS = '--enable-debug'
    }
    stages {
	stage('Build and test') {
	    stages {
                stage('Prep') {
                    steps {
                        sh 'sh autogen.sh'
                        sh './configure'
                    }
                }
                stage('Build') {
                    steps {
                        sh 'make $CONFIG_FLAGS'
                    }
                }
                stage('Test') {
                    steps {
			sh 'make check'
                    }
                }
                stage('Dist Check') {
                    steps {
			sh 'make distcheck'
                    }
                }
            }
	}
    }
}
