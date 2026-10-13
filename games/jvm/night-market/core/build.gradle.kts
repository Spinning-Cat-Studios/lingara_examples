// The game: the market's logic and its libGDX screen, on lingara-embed-kotlin.
// No natives here: the desktop launcher (../lwjgl3) adds them, and with them
// JNA, so the tests run with neither.

plugins {
    alias(libs.plugins.kotlin.jvm)
}

dependencies {
    api(libs.gdx)
    api(libs.gdx.freetype)
    api(libs.lingara.embed.kotlin)

    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
}
