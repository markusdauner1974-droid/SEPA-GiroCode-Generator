cmake_minimum_required(VERSION 3.22)
project(SepaGiroCodeGenerator LANGUAGES CXX)

find_package(Qt6 REQUIRED COMPONENTS Widgets Concurrent)
find_package(PkgConfig REQUIRED)
find_package(Threads REQUIRED)
pkg_check_modules(PDF REQUIRED IMPORTED_TARGET poppler-glib cairo cairo-gobject gio-2.0 gobject-2.0 glib-2.0)
find_library(TESSERACT_LIBRARY NAMES tesseract REQUIRED)
find_program(CARGO_EXECUTABLE cargo REQUIRED)

option(GIRO_OFFLINE "Build from vendored Rust dependencies" OFF)
if(GIRO_OFFLINE)
    set(CARGO_FLAGS --locked --offline)
endif()

set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")
set(RUST_LIBRARY "${CARGO_TARGET_DIR}/release/libsepa_girocode_generator.a")
file(GLOB RUST_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_custom_command(
    OUTPUT "${RUST_LIBRARY}"
    COMMAND "${CMAKE_COMMAND}" -E env "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
        "${CARGO_EXECUTABLE}" build --release --lib ${CARGO_FLAGS}
    DEPENDS ${RUST_SOURCES} "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    VERBATIM
)
add_custom_target(giro_core DEPENDS "${RUST_LIBRARY}")

qt_add_executable(rechnung-zu-girocode plasma/main.cpp plasma/preview.cpp)
add_dependencies(rechnung-zu-girocode giro_core)
target_compile_features(rechnung-zu-girocode PRIVATE cxx_std_17)
target_link_options(rechnung-zu-girocode PRIVATE "LINKER:--gc-sections")
target_link_libraries(rechnung-zu-girocode PRIVATE Qt6::Widgets Qt6::Concurrent
    "${RUST_LIBRARY}" PkgConfig::PDF "${TESSERACT_LIBRARY}" Threads::Threads ${CMAKE_DL_LIBS})
install(TARGETS rechnung-zu-girocode RUNTIME DESTINATION bin)

include(CTest)
set(CMAKE_AUTOMOC ON)
if(BUILD_TESTING)
    find_package(Qt6 REQUIRED COMPONENTS Test)
    qt_add_executable(giro-ui-test plasma/ui_test.cpp plasma/preview.cpp)
    add_dependencies(giro-ui-test giro_core)
    target_compile_features(giro-ui-test PRIVATE cxx_std_17)
    target_link_options(giro-ui-test PRIVATE "LINKER:--gc-sections")
    target_link_libraries(giro-ui-test PRIVATE Qt6::Widgets Qt6::Concurrent Qt6::Test
        "${RUST_LIBRARY}" PkgConfig::PDF "${TESSERACT_LIBRARY}" Threads::Threads ${CMAKE_DL_LIBS})
    add_test(NAME plasma-workflow COMMAND giro-ui-test)
    set_tests_properties(plasma-workflow PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
endif()
