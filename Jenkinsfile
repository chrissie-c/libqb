@Library('CCtestLib') _

pipeline {
    agent none
    stages {
       stage('general') {
          steps {
            runpipes(project:"libqb", branch:"main")
	  }
	}
    }
}
