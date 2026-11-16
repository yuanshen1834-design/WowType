# WowType Windows 移植：WinUI 候选窗 addon
add_library(winui MODULE winui.cpp)
target_include_directories(winui PRIVATE)
target_link_libraries(winui PRIVATE Fcitx5::Core Fcitx5::Utils user32 gdi32)
set_target_properties(winui PROPERTIES PREFIX "" OUTPUT_NAME "winui")

configure_file(winui.conf.in
    "${CMAKE_CURRENT_BINARY_DIR}/winui.conf" @ONLY)

install(TARGETS winui LIBRARY DESTINATION "${FCITX_INSTALL_ADDONDIR}")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/winui.conf"
    DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon")
