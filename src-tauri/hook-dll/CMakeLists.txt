cmake_minimum_required(VERSION 3.15)
project(MoonTranslatorHook LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Build as shared library (DLL)
add_library(moon_hook SHARED
    src/hook_text.cpp
)

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
)

# Install to src-tauri/bin for Tauri bundling
install(TARGETS moon_hook
    RUNTIME DESTINATION "${CMAKE_SOURCE_DIR}/../bin"
    LIBRARY DESTINATION "${CMAKE_SOURCE_DIR}/../bin"
)
