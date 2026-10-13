// The server half: lingara-embed-server-kotlin over the JDK's own HTTP
// server, so it adds no web framework. `./gradlew :server:run` serves it.
//
// lingara-kotlin is named directly: the server builds the client the kit
// takes, and builds it with the library's own builder.

plugins {
    alias(libs.plugins.kotlin.jvm)
    application
}

dependencies {
    implementation(libs.lingara.embed.server.kotlin)
    implementation(libs.lingara.kotlin)
    implementation(libs.kotlinx.coroutines.core)

    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
}

application {
    mainClass = "nightmarket.server.ServerKt"
}
