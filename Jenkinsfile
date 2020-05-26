pipeline {
  agent none
  parameters {
    gitParameter name: 'TAG', 
                 type: 'PT_TAG',
                 defaultValue: 'master'
  }

  stages {
    stage('Checkout Git TAG (on dedicated agent)') {
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
      }
    }

    stage('Native libs build') {
      agent { label 'UbuntuVM' }
      steps {
        sh '''
        git log --graph --date=short --pretty=tformat:'%ad - %h - %cn -%d %s' -n 20 || true
        cd zenoh
        mvn -Prelease generate-sources
        '''
        stash includes: 'zenoh/target/generated-sources/**/*.java, zenoh/target/resources/**/*zenohc_java.*' name: 'nativeLibs'
      }
    }

    stage('Release build') {
      agent any
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
        mvn -Djipp -Prelease install
        '''
      }
    }
  }

  post {
    success {
        archiveArtifacts artifacts: 'zenoh/target/zenoh-*.jar, examples/*/target/zenoh-*.jar', fingerprint: true
    }
  }
}
