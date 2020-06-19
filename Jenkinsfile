pipeline {
  agent any
  parameters {
    gitParameter name: 'TAG', 
                 type: 'PT_TAG',
                 defaultValue: 'Jenkins_tests'
  }

  stages {
    stage('Native libs build (on dedicated agent)') {
      agent { label 'MacMini' }
      steps {
        cleanWs()
        checkout scm
        sh '''
          source ~/.zshrc
          eval "$(jenv init -)"

          git log --graph --date=short --pretty=tformat:'%ad - %h - %cn -%d %s' -n 20 || true
          echo $PATH
          echo $JAVA_HOME
          cd zenoh
          mvn -Prelease generate-sources
        '''
        stash includes: 'zenoh/target/generated-sources/**/*.java, zenoh/target/resources/**/*zenohc_java.*', name: 'nativeLibs'
      }
    }

    stage('Release build') {
      tools {
          maven 'apache-maven-latest'
          jdk 'adoptopenjdk-hotspot-jdk8-latest'
      }
      steps {
        cleanWs()
        checkout scm
        unstash 'nativeLibs'
        sh '''
          ls -al zenoh/target/generated-sources/java/org/eclipse/zenoh/swig/
          ls -al zenoh/target/resources/natives/*
          ls -al ~/.m2/repository
          ls -al /home/jenkins/.m2
          ls -al /home/jenkins/.m2/repository
        '''
        withCredentials([file(credentialsId: 'secret-subkeys.asc', variable: 'KEYRING')]) {
          sh 'gpg --batch --import "${KEYRING}"'
          sh 'for fpr in $(gpg --list-keys --with-colons  | awk -F: \'/fpr:/ {print $10}\' | sort -u); do echo -e "5\ny\n" |  gpg --batch --command-fd 0 --expert --edit-key ${fpr} trust; done'
        }
        sh 'mvn -Djipp -Prelease deploy'
      }
    }
  }

  post {
    success {
        archiveArtifacts artifacts: 'zenoh/target/zenoh-*.jar, examples/*/target/zenoh-*.jar', fingerprint: true
    }
  }
}
