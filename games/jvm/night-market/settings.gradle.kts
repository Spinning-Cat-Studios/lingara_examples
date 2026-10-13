// The Night Market, on the JVM: a libGDX desktop game and its Kotlin server,
// built only from Maven Central. Three modules in one build, each with its own
// committed gradle.lockfile.
//
// Repositories by role: plugins from the Gradle Plugin Portal or Maven
// Central, every dependency from Maven Central alone, and a module that names
// a repository of its own fails the build.

pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        mavenCentral()
    }
}

rootProject.name = "night-market"

include("core", "lwjgl3", "server")
