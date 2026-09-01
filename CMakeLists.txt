cmake_minimum_required(VERSION 3.20)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

project(lsaca VERSION 0.0.1)

set(GCC_FLAGS
    -Wall
    -Wextra
    -Wpedantic
    -Wshadow
    -Wstrict-prototypes
    -Wmissing-prototypes
    -Wold-style-definition
    -Wmissing-declarations
    -Wredundant-decls
    -Wnested-externs
    -Wpointer-arith
    -Wcast-align
    -Wwrite-strings
    -Wformat=2
    -Wformat-overflow=2
    -Wformat-truncation=2
    -Wimplicit-fallthrough
    -Wmissing-include-dirs
    -Wswitch-default
    -Wuninitialized
    -Wmaybe-uninitialized
    -Wstrict-overflow=4
    -Wduplicated-branches
    -Wduplicated-cond
    -Wnull-dereference
    -Wjump-misses-init
    -Wlogical-op
    -Wno-unused-parameter
    -Wno-missing-field-initializers
)

set(CLANG_FLAGS
    -Wall
    -Wextra
    -Wpedantic
    -Wshadow
    -Wstrict-prototypes
    -Wmissing-prototypes
    -Wold-style-definition
    -Wmissing-declarations
    -Wredundant-decls
    -Wpointer-arith
    -Wcast-align
    -Wwrite-strings
    -Wformat=2
    -Wimplicit-fallthrough
    -Wswitch-default
    -Wuninitialized
    -Wnull-dereference
    -Wno-unused-parameter
    -Wno-missing-field-initializers
    -Wno-newline-eof -Wno-format-nonliteral
)

set(MSVC_FLAGS
    /W4
    /wd4200
    /wd4201
    /wd4204
    /wd4221
    /wd4996
)

add_executable(${PROJECT_NAME}
    src/base/lsaca_any.c
    src/base/lsaca_array.c
    src/base/lsaca_gc.c
    src/base/lsaca_map.c
    src/base/lsaca_string.c
    src/lsaca_chunk.c
    src/lsaca_compiler.c
    src/lsaca_disasm.c
    src/lsaca_scanner.c
    src/lsaca_vm.c
    src/main.c
)

target_include_directories(${PROJECT_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})

if(CMAKE_C_COMPILER_ID STREQUAL "GCC")
    target_compile_options(${PROJECT_NAME} PRIVATE ${GCC_FLAGS}
        $<$<CONFIG:Debug>:-O0;-g3;-ggdb;-fno-omit-frame-pointer>
        $<$<CONFIG:Release>:-O3;-DNDEBUG;-flto;-fdata-sections;-ffunction-sections>)
    target_link_options(${PROJECT_NAME} PRIVATE
        $<$<CONFIG:Release>:-flto;-Wl,--gc-sections>)
elseif(CMAKE_C_COMPILER_ID STREQUAL "Clang")
    target_compile_options(${PROJECT_NAME} PRIVATE ${CLANG_FLAGS}
        $<$<CONFIG:Debug>:-O0;-g3;-fno-omit-frame-pointer>
        $<$<CONFIG:Release>:-O3;-DNDEBUG;-flto=thin;-fdata-sections;-ffunction-sections>)
    target_link_options(${PROJECT_NAME} PRIVATE
        $<$<CONFIG:Release>:-flto=thin>)
elseif(CMAKE_C_COMPILER_ID STREQUAL "MSVC")
    target_compile_options(${PROJECT_NAME} PRIVATE ${MSVC_FLAGS}
        $<$<CONFIG:Debug>:/Od;/Zi>
        $<$<CONFIG:Release>:/O2;/GL;/GS-;/Gy>)
    target_link_options(${PROJECT_NAME} PRIVATE
        $<$<CONFIG:Release>:/LTCG;/OPT:REF>)
endif()