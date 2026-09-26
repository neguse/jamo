set(JAMO_TEST_DICT "${JAMO_ROOT}/data/mozc.dict" CACHE FILEPATH "dictionary for the E2E test")
configure_file(testdir.h.in "${CMAKE_CURRENT_BINARY_DIR}/testdir.h" @ONLY)

# fcitx5 はテスト時 ${CMAKE_BINARY_DIR}/test/{addon,inputmethod} から設定を読む
file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/test/addon" "${CMAKE_BINARY_DIR}/test/inputmethod")
add_custom_target(jamo-test-conf
    COMMAND ${CMAKE_COMMAND} -E copy "${PROJECT_BINARY_DIR}/src/jamo-addon.conf"
            "${CMAKE_BINARY_DIR}/test/addon/jamo.conf"
    COMMAND ${CMAKE_COMMAND} -E copy "${PROJECT_SOURCE_DIR}/src/jamo.conf"
            "${CMAKE_BINARY_DIR}/test/inputmethod/jamo.conf")

add_executable(testjamo testjamo.cpp)
target_include_directories(testjamo PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(testjamo Fcitx5::Core Fcitx5::Module::TestFrontend)
add_dependencies(testjamo jamo jamo-test-conf)
add_test(NAME testjamo COMMAND testjamo)
set_tests_properties(testjamo PROPERTIES SKIP_RETURN_CODE 77)
