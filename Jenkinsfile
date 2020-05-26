pipeline {
  agent any
  parameters {
    gitParameter name: 'TAG', 
                 type: 'PT_TAG',
                 defaultValue: 'master'
  }

  stages {
    stage('Native libs build (on dedicated agent)') {
      agent { label 'UbuntuVM' }
      steps {
        cleanWs()
        checkout([$class: 'GitSCM',
                  branches: [[name: "${params.TAG}"]],
                  doGenerateSubmoduleConfigurations: false,
                  extensions: [],
                  gitTool: 'Default',
                  submoduleCfg: [],
                  userRemoteConfigs: [[url: 'https://github.com/atolab/eclipse-zenoh-java.git']]
                ])
        sh '''
          git log --graph --date=short --pretty=tformat:'%ad - %h - %cn -%d %s' -n 20 || true
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
        checkout([$class: 'GitSCM',
                  branches: [[name: "${params.TAG}"]],
                  doGenerateSubmoduleConfigurations: false,
                  extensions: [],
                  gitTool: 'Default',
                  submoduleCfg: [],
                  userRemoteConfigs: [[url: 'https://github.com/atolab/eclipse-zenoh-java.git']]
                ])
        unstash 'nativeLibs'
        sh '''
          ls -al zenoh/target/generated-sources/java/org/eclipse/zenoh/swig/
          ls -al zenoh/target/resources/natives/*
        '''
        withCredentials([file(credentialsId: 'secret-subkeys.asc', variable: 'KEYRING')]) {
          sh 'gpg --batch --import "${KEYRING}"'
          sh 'for fpr in $(gpg --list-keys --with-colons  | awk -F: \'/fpr:/ {print $10}\' | sort -u); do echo -e "5\ny\n" |  gpg --batch --command-fd 0 --expert --edit-key ${fpr} trust; done'
        }
        sh 'mvn -Djipp -Prelease install'
      }
    }
  }

  post {
    success {
        archiveArtifacts artifacts: 'zenoh/target/zenoh-*.jar, examples/*/target/zenoh-*.jar', fingerprint: true
    }
  }
}
