// What every module shares: dependency locking, and bytecode for Java 17.
//
// Any JDK 17 or newer builds this; there is no toolchain to provision, so the
// release flags below are what keep the bytecode at 17.
//
// Relock every module in one command after a version changes:
//     ./gradlew resolveAndLockAll --write-locks

import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension

plugins {
    alias(libs.plugins.kotlin.jvm) apply false
}

subprojects {
    dependencyLocking {
        lockAllConfigurations()
    }

    // Gradle's locking guide: resolve every resolvable configuration, so
    // --write-locks records each one in this module's gradle.lockfile.
    tasks.register("resolveAndLockAll") {
        notCompatibleWithConfigurationCache("Filters configurations at execution time")
        doFirst {
            require(gradle.startParameter.isWriteDependencyLocks) { "run resolveAndLockAll with --write-locks" }
        }
        doLast {
            configurations.filter { it.isCanBeResolved }.forEach { it.resolve() }
        }
    }

    plugins.withId("org.jetbrains.kotlin.jvm") {
        extensions.configure<KotlinJvmProjectExtension> {
            compilerOptions {
                jvmTarget = JvmTarget.JVM_17
                freeCompilerArgs.add("-Xjdk-release=17")
            }
        }
        tasks.withType<JavaCompile>().configureEach {
            options.release = 17
        }
        tasks.withType<Test>().configureEach {
            useJUnitPlatform()
        }
    }
}
