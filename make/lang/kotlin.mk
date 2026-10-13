# Kotlin examples (ADR 13.10.26i D6). A Kotlin example is one Gradle build
# with its wrapper committed, so every target runs the example's own
# ./gradlew in its directory and needs only a JDK 17 or newer. Each module
# locks its configurations in its own gradle.lockfile.
#
# No `live-example-kotlin`: a game needs a player at the keyboard, so a
# Kotlin row is `live = false` (roadmap 13.10.26g D3).

.PHONY: build-example-kotlin test-example-kotlin lock-example-kotlin

build-example-kotlin:
	cd "$(DIR)" && ./gradlew --no-daemon build -x test

test-example-kotlin:
	cd "$(DIR)" && ./gradlew --no-daemon test

# After `bump` wrote the new catalogue version: one command relocks every
# module, so PKG is accepted and unused.
lock-example-kotlin:
	cd "$(DIR)" && ./gradlew --no-daemon resolveAndLockAll --write-locks
