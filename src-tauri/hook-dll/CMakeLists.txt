cmake_minimum_required(VERSION 3.15)
project(MoonTranslatorHook LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Build as shared library (DLL)
add_library(moon_hook SHARED
    src/hook_text.cpp
)

# /EHa: enable SEH (__try/__except) + C++ exception handling to coexist.
# hook_text.cpp uses __try around C++ objects (std::string) — /EHsc (CMake
# default) would emit C2712 "cannot use __try in functions that require
# object unwinding". /EHa lets SEH and C++ EH mix in the same function.
#
# CMAKE_CXX_FLAGS inherits /EHsc from the MSVC platform module; that flag is
# emitted AFTER target_compile_options on the cl.exe command line and would
# override /EHa (last /EH wins). We strip it so /EHa is the only EH flag.
if(MSVC)
    string(REPLACE "/EHsc" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
    target_compile_options(moon_hook PRIVATE /EHa)
endif()

# Set output name
set_target_properties(moon_hook PROPERTIES
    OUTPUT_NAME "moon_hook"
    PREFIX ""
    SUFFIX ".dll"
)

# Link Windows libraries
target_link_libraries(moon_hook PRIVATE
    kernel32
    user32
    gdi32
    psapi
)

# Install to src-tauri/bin for Tauri bundling
install(TARGETS moon_hook
    RUNTIME DESTINATION "${CMAKE_SOURCE_DIR}/../bin"
    LIBRARY DESTINATION "${CMAKE_SOURCE_DIR}/../bin"
)
