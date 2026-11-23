import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Release signing: keystore.properties next to this project's settings (never committed),
// with storeFile, password and keyAlias. Without it, release builds come out unsigned.
val keystoreProperties = Properties().apply {
    val propFile = rootProject.file("keystore.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 37
    if (keystoreProperties.containsKey("storeFile")) {
        signingConfigs {
            create("release") {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("password")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("password")
            }
        }
    }
    namespace = "com.alwy.chat"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        // The app's id on phones (its owner's domain, alwyapps.be). The code keeps its own package
        // (`namespace` above, which the Rust side's JNI names follow) and the push gateway keeps
        // knowing the app as com.alwy.chat (alwy_core::names::APP_ID): neither is tied to this id.
        // ALWY_ANDROID_ID: builds published outside GitHub can keep another id (release-local.sh).
        applicationId = providers.environmentVariable("ALWY_ANDROID_ID").getOrElse("be.alwyapps.chat")
        minSdk = 24
        targetSdk = 37
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            signingConfigs.findByName("release")?.let { signingConfig = it }
            // The Rust library compresses ~2.5× (51 → 21 MB): a much smaller download and
            // update, unpacked once at install instead of being stored uncompressed in the APK.
            packaging {
                jniLibs.useLegacyPackaging = true
            }
            optimization {
               enable = true
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
}

// Google Play builds (ALWY_PLAY=1, scripts/play-build.sh): the store updates the app, so the
// permission to install updates is left out (Play refuses apps that ask for it).
if (providers.environmentVariable("ALWY_PLAY").isPresent) {
    android.sourceSets.getByName("release").manifest.srcFile("src/play/AndroidManifest.xml")
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

// The Kotlin half of rustls-platform-verifier must match the crate version in Cargo.lock.
val rustlsPlatformVerifierVersion: String = rootProject.file("../../../../Cargo.lock").readLines().let { lines ->
    val nameIdx = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
    lines.drop(nameIdx + 1).first { it.trimStart().startsWith("version = ") }
        .substringAfter('"').substringBefore('"')
}

dependencies {
    implementation("org.rustls:rustls-platform-verifier:$rustlsPlatformVerifierVersion")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    // Push (FCM). Without google-services.json the app builds and notifies while running only.
    implementation(platform("com.google.firebase:firebase-bom:34.5.0"))
    implementation("com.google.firebase:firebase-messaging")
    // Push without Google: UnifiedPush distributors (ntfy…).
    implementation("org.unifiedpush.android:connector:3.3.5")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))

// Firebase project config (not committed: each build/fork uses its own Firebase project).
if (file("google-services.json").exists()) {
    apply(plugin = "com.google.gms.google-services")
}
