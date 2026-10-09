cmake_minimum_required(VERSION 3.22)
include(cmake/arm-gcc.cmake)
project(dm_h723_usb_can LANGUAGES C ASM)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "" FORCE)
endif()
set(CPU_FLAGS -mcpu=cortex-m7 -mthumb -mfpu=fpv5-d16 -mfloat-abi=hard)
file(GLOB HAL_SOURCES CONFIGURE_DEPENDS vendor/HAL/Src/*.c)
file(GLOB USB_SOURCES CONFIGURE_DEPENDS vendor/USB/Core/Src/*.c vendor/USB/CDC/Src/*.c)
list(FILTER USB_SOURCES EXCLUDE REGEX "_template\\.c$")
add_library(st_drivers OBJECT ${HAL_SOURCES} ${USB_SOURCES}
  Core/Src/system_stm32h7xx.c Core/Src/startup_stm32h723xx.s
  USB_DEVICE/App/usb_device.c USB_DEVICE/App/usbd_desc.c USB_DEVICE/Target/usbd_conf.c)
add_executable(firmware.elf Core/Src/main.c Core/Src/board.c Core/Src/protocol.c
  Core/Src/bridge.c Core/Src/motor.c Core/Src/motor_can.c USB_DEVICE/App/usbd_cdc_if.c)
set(INCLUDES Core/Inc USB_DEVICE/App USB_DEVICE/Target vendor/HAL/Inc
  vendor/CMSIS/Include vendor/CMSIS/Device/Include vendor/USB/Core/Inc vendor/USB/CDC/Inc)
foreach(target st_drivers firmware.elf)
  target_include_directories(${target} PRIVATE ${INCLUDES})
  target_compile_definitions(${target} PRIVATE USE_HAL_DRIVER STM32H723xx USE_PWR_LDO_SUPPLY
    USER_VECT_TAB_ADDRESS HSE_VALUE=24000000UL)
  target_compile_options(${target} PRIVATE ${CPU_FLAGS} -ffunction-sections -fdata-sections
    $<$<COMPILE_LANGUAGE:C>:-Wall;-Wextra>)
endforeach()
target_compile_options(firmware.elf PRIVATE $<$<COMPILE_LANGUAGE:C>:-Werror>)
target_link_libraries(firmware.elf PRIVATE st_drivers)
target_link_options(firmware.elf PRIVATE ${CPU_FLAGS} -nostartfiles -T${CMAKE_CURRENT_SOURCE_DIR}/STM32H723VG_FLASH.ld
  --specs=nano.specs --specs=nosys.specs -Wl,--gc-sections -Wl,--print-memory-usage
  -Wl,-Map=${CMAKE_CURRENT_BINARY_DIR}/firmware.map)
set_property(TARGET firmware.elf APPEND PROPERTY LINK_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/STM32H723VG_FLASH.ld)
add_custom_command(TARGET firmware.elf POST_BUILD
  COMMAND ${CMAKE_OBJCOPY} -O ihex $<TARGET_FILE:firmware.elf> firmware.hex
  COMMAND ${CMAKE_OBJCOPY} -O binary $<TARGET_FILE:firmware.elf> firmware.bin
  COMMAND ${CMAKE_SIZE} $<TARGET_FILE:firmware.elf>)
