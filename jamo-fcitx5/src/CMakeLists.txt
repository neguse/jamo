add_library(jamo MODULE engine.cpp)
set_target_properties(jamo PROPERTIES PREFIX "")
target_link_libraries(jamo Fcitx5::Core jamo_core)
target_link_options(jamo PRIVATE -Wl,--exclude-libs,ALL)
install(TARGETS jamo DESTINATION "${CMAKE_INSTALL_LIBDIR}/fcitx5")

configure_file(jamo-addon.conf.in jamo-addon.conf @ONLY)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/jamo-addon.conf" RENAME jamo.conf
        DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/addon")
install(FILES jamo.conf DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/inputmethod")
