// SPDX-FileCopyrightText: 2023 perillamint
//
// SPDX-License-Identifier: CC0-1.0

pipeline {
    agent none
    stages {
        stage('Test') {
            agent {
                docker {
                    image 'ghcr.io/cleanc-lab/rust:1.72.0-slim-bookworm' 
                    args '--privileged --net=host -v /var/run/docker.sock:/var/run/docker.sock'
                }
            }
            stages {
                stage('Lint') {
                    steps {
                        sh 'reuse lint'
                        sh 'cargo fmt --all -- --check'
                        sh 'cargo clippy --all-targets --all-features -- -Dclippy::all'
                    }
                }
                stage('Build') {
                    steps {
                        sh 'cargo build'
                    }
                }
                stage('Nextest') {
                    steps {
                        sh '/usr/local/cargo/bin/cargo-nextest nextest run'
                    }
                }
            }
        }
        stage('Docker') {
            agent {
                docker {
                    image 'docker:24-cli'
                    args '--privileged -v /var/run/docker.sock:/var/run/docker.sock'
                }
            }
            when {
                anyOf {
                    branch 'master';
                    buildingTag();
                }
            }
            environment {
                DOCKER_REGISTRY = 'ghcr.io'
                GITHUB_ORG = 'cybersiliconforest'
                DOCKER_IMAGE = "${env.DOCKER_REGISTRY}/${env.GITHUB_ORG}/portalgun_moon"
                GHCR_TOKEN = credentials('siliconforest-jenkins-github-pat-package-rw')
            }
            stages {
                stage('Prepare') {
                    steps {
                        script {
                            if (env.BRANCH_NAME == 'master') {
                                env.DOCKER_TAG = 'develop'
                                env.DOCKER_LATEST = 'false'
                            } else {
                                env.DOCKER_TAG = env.TAG_NAME
                                env.DOCKER_LATEST = 'true'
                            }
                        }
                    }
                }
                stage('Docker login') {
                    steps {
                        sh 'echo $GHCR_TOKEN_PSW | docker login ghcr.io -u $GHCR_TOKEN_USR --password-stdin'
                    }
                }
                stage('Build') {
                    matrix {
                        axes {
                            axis {
                                name 'TARGET'
                                //values 'amd64', 'arm64'
                                values 'amd64'
                            }
                        }
                        stages {
                            stage('Build platform specific image') {
                                steps {
                                    sh "docker build -t $DOCKER_IMAGE:$DOCKER_TAG-${TARGET} --platform linux/${TARGET} ."
                                }
                            }
                            stage('Push platform specific image') {
                                steps {
                                    sh "docker push $DOCKER_IMAGE:$DOCKER_TAG-${TARGET}"
                                }
                            }
                        }
                    }
                }
                stage('Docker manifest') {
                    steps {
                        //sh "docker manifest create $DOCKER_IMAGE:$DOCKER_TAG --amend $DOCKER_IMAGE:$DOCKER_TAG-amd64 --amend $DOCKER_IMAGE:$DOCKER_TAG-arm64"
                        sh "docker manifest create $DOCKER_IMAGE:$DOCKER_TAG --amend $DOCKER_IMAGE:$DOCKER_TAG-amd64"
                        script {
                            if (env.DOCKER_LATEST == 'true') {
                                //sh "docker manifest create $DOCKER_IMAGE:latest --amend $DOCKER_IMAGE:$DOCKER_TAG-amd64 --amend $DOCKER_IMAGE:$DOCKER_TAG-arm64"
                                sh "docker manifest create $DOCKER_IMAGE:latest --amend $DOCKER_IMAGE:$DOCKER_TAG-amd64"
                            }
                        }
                    }
                }
                stage('Docker push') {
                    steps {
                        sh "docker manifest push $DOCKER_IMAGE:$DOCKER_TAG"
                        script {
                            if (env.DOCKER_LATEST == 'true') {
                                sh "docker manifest push $DOCKER_IMAGE:latest"
                            }
                        }
                    }
                }
            }
            post {
                always {
                    sh 'docker logout "$DOCKER_REGISTRY"'
                }
            }
        }
    }
}
