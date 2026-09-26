cmake_minimum_required(VERSION 3.21)
project(jamo-fcitx5 VERSION 0.2.0 LANGUAGES CXX)

find_package(Fcitx5Core 5.1.12 REQUIRED)
find_package(Fcitx5Module REQUIRED COMPONENTS TestFrontend)
find_package(Threads REQUIRED)
include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin")
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin")

# jamo-core は cargo でビルドした staticlib をリンクする
set(JAMO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/..")
set(JAMO_CORE_LIB "${JAMO_ROOT}/target/release/libjamo_core.a")
add_custom_command(
    OUTPUT "${JAMO_CORE_LIB}"
    COMMAND cargo build --release -p jamo-core
    WORKING_DIRECTORY "${JAMO_ROOT}"
    DEPENDS "${JAMO_ROOT}/jamo-core/Cargo.toml"
    COMMENT "cargo build jamo-core"
    USES_TERMINAL)
add_custom_target(jamo-core-build DEPENDS "${JAMO_CORE_LIB}")
add_library(jamo_core STATIC IMPORTED)
set_target_properties(jamo_core PROPERTIES
    IMPORTED_LOCATION "${JAMO_CORE_LIB}"
    INTERFACE_INCLUDE_DIRECTORIES "${JAMO_ROOT}/jamo-core/include"
    INTERFACE_LINK_LIBRARIES "Threads::Threads;${CMAKE_DL_LIBS};m")
add_dependencies(jamo_core jamo-core-build)

add_subdirectory(src)

enable_testing()
add_subdirectory(test)
