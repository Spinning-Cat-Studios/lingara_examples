// The desktop launcher: libGDX's LWJGL3 backend, and the native libraries
// for libGDX, FreeType and Lingara. `./gradlew :lwjgl3:run` starts the game.

plugins {
    alias(libs.plugins.kotlin.jvm)
    application
}

dependencies {
    implementation(project(":core"))
    implementation(libs.gdx.backend.lwjgl3)
    implementation(variantOf(libs.gdx.platform) { classifier("natives-desktop") })
    implementation(variantOf(libs.gdx.freetype.platform) { classifier("natives-desktop") })
    // `implementation`, not `runtimeOnly`: it also brings JNA, which the kit's types refer to.
    implementation(libs.lingara.embed.natives.desktop)
}

application {
    mainClass = "nightmarket.LauncherKt"
    // GLFW must own the main thread on macOS.
    if (System.getProperty("os.name").startsWith("Mac")) {
        applicationDefaultJvmArgs = listOf("-XstartOnFirstThread")
    }
}
